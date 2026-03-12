use super::AppState;
use crate::core::{NodeId, value::Value, value_path::ValueTarget};
use crate::widgets::node::{NodeWalkScope, find_node, walk_nodes_mut};
use std::collections::HashSet;

impl AppState {
    pub(super) fn sync_current_step_values_to_store(&mut self) {
        let changed_targets = self.refresh_current_step_bindings_collect();
        self.emit_store_change_triggers(changed_targets);
    }

    pub(super) fn refresh_current_step_bindings(&mut self) {
        let changed_targets = self.refresh_current_step_bindings_collect();
        self.emit_store_change_triggers(changed_targets);
    }

    fn refresh_current_step_bindings_collect(&mut self) -> Vec<ValueTarget> {
        let mut commit = BindingCommit::default();
        self.bootstrap_missing_current_step_value_bindings(&mut commit);
        self.commit_focused_current_step_writes(&mut commit);
        self.hydrate_current_step_from_store();
        self.propagate_current_step_derived_writes(&mut commit);
        self.hydrate_current_step_from_store();
        commit.finish()
    }

    pub(super) fn apply_value_change(&mut self, target: impl Into<NodeId>, value: Value) {
        self.write_value_direct(ValueTarget::node(target.into()), value);
    }

    pub(super) fn apply_value_change_target(&mut self, target: ValueTarget, value: Value) {
        self.write_value_direct(target, value);
    }

    fn write_value_direct(&mut self, target: ValueTarget, value: Value) {
        let root = target.root().clone();
        let direct_target = target.clone();
        let changed = match self.write_store_target(target, value) {
            Ok(changed) => changed,
            Err(err) => {
                self.runtime
                    .validation
                    .set_runtime_step_error(store_write_error_key(root.as_str()), err.to_string());
                return;
            }
        };
        self.runtime
            .validation
            .clear_runtime_step_error(store_write_error_key(root.as_str()).as_str());
        let mut commit = BindingCommit::default();
        if changed {
            commit.record_target(direct_target);
        }
        if !self.reconcile_current_step_after_store_change() {
            commit.extend(self.refresh_current_step_bindings_collect());
        }
        self.emit_store_change_triggers(commit.finish());
    }

    pub(super) fn hydrate_current_step_from_store(&mut self) -> bool {
        let mut changed = false;
        let store = &self.data.store;
        let focused_id = self.ui.focus.current_id().map(str::to_string);
        walk_nodes_mut(
            self.flow.current_step_mut().nodes.as_mut_slice(),
            NodeWalkScope::Recursive,
            &mut |node| {
                let is_focused = focused_id
                    .as_deref()
                    .is_some_and(|focused_id| node.id() == focused_id);
                changed |= node.sync_from_store_with_focus(store, is_focused);
            },
        );
        changed
    }

    fn bootstrap_missing_current_step_value_bindings(&mut self, commit: &mut BindingCommit) {
        let bindings = self
            .flow
            .current_step()
            .binding_plan
            .direct_value_nodes
            .clone();
        for binding in bindings {
            if self.data.store.get_target(&binding.target).is_some() {
                continue;
            }
            let nodes = self.flow.current_step().nodes.as_slice();
            let Some(value) =
                find_node(nodes, binding.node_id.as_str()).and_then(|node| node.value())
            else {
                continue;
            };
            commit.apply_write(self, binding.target, value);
        }
    }

    fn commit_focused_current_step_writes(&mut self, commit: &mut BindingCommit) {
        let Some(focused_id) = self.ui.focus.current_id().map(str::to_string) else {
            return;
        };
        let nodes = self.flow.current_step().nodes.as_slice();
        let Some(changes) = find_node(nodes, focused_id.as_str()).map(|node| node.write_changes())
        else {
            return;
        };
        for change in changes {
            commit.apply_write(self, change.target, change.value);
        }
    }

    fn propagate_current_step_derived_writes(&mut self, commit: &mut BindingCommit) {
        let derived_ids = self
            .flow
            .current_step()
            .binding_plan
            .derived_write_nodes
            .clone();
        for node_id in derived_ids {
            let nodes = self.flow.current_step().nodes.as_slice();
            let Some(changes) = find_node(nodes, node_id.as_str()).map(|node| node.write_changes())
            else {
                continue;
            };
            let mut node_changed = false;
            for change in changes {
                node_changed |= commit.apply_write(self, change.target, change.value);
            }
            if node_changed {
                self.hydrate_current_step_from_store();
            }
        }
    }

    fn write_store_target(
        &mut self,
        target: ValueTarget,
        value: Value,
    ) -> Result<bool, crate::state::store::StoreWriteError> {
        let root = target.root().clone();
        let before = self.data.store.get(root.as_str()).cloned();
        self.data.store.set_target(&target, value)?;
        self.runtime
            .validation
            .clear_runtime_step_error(store_write_error_key(root.as_str()).as_str());
        let updated = self.data.store.get(root.as_str()).cloned();
        Ok(updated
            .as_ref()
            .is_some_and(|updated| before.as_ref() != Some(updated)))
    }

    fn emit_store_change_triggers(&mut self, targets: Vec<ValueTarget>) {
        for target in targets {
            crate::task::engine::trigger_store_value_changed_tasks(self, &target);
        }
    }
}

#[derive(Default)]
struct BindingCommit {
    changed_targets: Vec<ValueTarget>,
    seen_targets: HashSet<String>,
}

impl BindingCommit {
    fn apply_write(&mut self, state: &mut AppState, target: ValueTarget, value: Value) -> bool {
        match state.write_store_target(target.clone(), value) {
            Ok(changed) => {
                if changed {
                    self.record_target(target);
                }
                changed
            }
            Err(err) => {
                let root = err_root_key(&err);
                state
                    .runtime
                    .validation
                    .set_runtime_step_error(store_write_error_key(root.as_str()), err.to_string());
                false
            }
        }
    }

    fn record_target(&mut self, target: ValueTarget) {
        let selector = target.to_selector();
        if self.seen_targets.insert(selector) {
            self.changed_targets.push(target);
        }
    }

    fn extend(&mut self, targets: Vec<ValueTarget>) {
        for target in targets {
            self.record_target(target);
        }
    }

    fn finish(self) -> Vec<ValueTarget> {
        self.changed_targets
    }
}

fn err_root_key(err: &crate::state::store::StoreWriteError) -> String {
    match err {
        crate::state::store::StoreWriteError::RootTypeConflict { root, .. } => root.clone(),
        crate::state::store::StoreWriteError::PathTypeConflict { target, .. } => {
            ValueTarget::parse_selector(target)
                .map(|target| target.root().to_string())
                .unwrap_or_else(|_| target.clone())
        }
    }
}

fn store_write_error_key(root: &str) -> String {
    format!("store:{root}")
}
