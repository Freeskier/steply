use super::AppState;
use crate::core::{NodeId, value::Value, value_path::ValueTarget};
use crate::runtime::event::ValueChange;
use crate::widgets::node::{NodeWalkScope, walk_nodes, walk_nodes_mut};
use std::collections::HashMap;

const MAX_BINDING_SETTLE_PASSES: usize = 8;

impl AppState {
    pub(super) fn sync_current_step_values_to_store(&mut self) {
        let _ = self.push_current_step_writes_to_store();
    }

    pub(super) fn settle_current_step_bindings(&mut self) {
        for _ in 0..MAX_BINDING_SETTLE_PASSES {
            let hydrated = self.hydrate_current_step_from_store();
            let store_changed = self.push_current_step_writes_to_store();
            if !store_changed && !hydrated {
                break;
            }
        }
    }

    pub(super) fn apply_value_change(&mut self, target: impl Into<NodeId>, value: Value) {
        self.write_value_direct(ValueTarget::node(target.into()), value);
    }

    pub(super) fn apply_value_change_target(&mut self, target: ValueTarget, value: Value) {
        self.write_value_direct(target, value);
    }

    fn write_value_direct(&mut self, target: ValueTarget, value: Value) {
        let root = target.root().clone();
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
        if !self.reconcile_current_step_after_store_change() {
            self.settle_current_step_bindings();
        }
        if changed && let Some(updated) = self.data.store.get(root.as_str()).cloned() {
            crate::task::engine::trigger_node_value_changed_tasks(self, root.as_str(), &updated);
        }
    }

    pub(super) fn hydrate_current_step_from_store(&mut self) -> bool {
        let mut changed = false;
        let store = &self.data.store;
        walk_nodes_mut(
            self.flow.current_step_mut().nodes.as_mut_slice(),
            NodeWalkScope::Recursive,
            &mut |node| changed |= node.sync_from_store(store),
        );
        changed
    }

    fn push_current_step_writes_to_store(&mut self) -> bool {
        let focused_id = self.ui.focus.current_id().map(str::to_string);
        let changes = {
            let mut out = Vec::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Recursive,
                &mut |node| {
                    let is_focused = focused_id
                        .as_deref()
                        .is_some_and(|focused_id| node.id() == focused_id);
                    let has_reads = node
                        .store_binding()
                        .and_then(|binding| binding.reads.as_ref())
                        .is_some();
                    let base_order = out.len();
                    for (idx, change) in node.write_changes().into_iter().enumerate() {
                        out.push(PendingWrite {
                            change,
                            is_focused,
                            has_reads,
                            order: base_order + idx,
                        });
                    }
                },
            );
            out
        };
        let mut selected = HashMap::<String, PendingWrite>::new();
        for pending in changes {
            let key = pending.change.target.to_selector();
            match selected.get(key.as_str()) {
                Some(existing) if !pending.should_replace(existing) => {}
                _ => {
                    selected.insert(key, pending);
                }
            }
        }
        let mut store_changed = false;
        let mut changes = selected.into_values().collect::<Vec<_>>();
        changes.sort_by_key(|pending| pending.order);
        for pending in changes {
            let change = pending.change;
            match self.write_store_target(change.target, change.value) {
                Ok(changed) => {
                    store_changed |= changed;
                }
                Err(err) => {
                    let root = err_root_key(&err);
                    self.runtime.validation.set_runtime_step_error(
                        store_write_error_key(root.as_str()),
                        err.to_string(),
                    );
                }
            }
        }
        store_changed
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
}

struct PendingWrite {
    change: ValueChange,
    is_focused: bool,
    has_reads: bool,
    order: usize,
}

impl PendingWrite {
    fn priority(&self) -> u8 {
        if self.is_focused {
            2
        } else if self.has_reads {
            0
        } else {
            1
        }
    }

    fn should_replace(&self, other: &Self) -> bool {
        self.priority() > other.priority()
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
