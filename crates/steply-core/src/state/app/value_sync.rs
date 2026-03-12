use super::AppState;
use super::transaction::AppliedStorePatch;
use crate::core::{NodeId, value::Value, value_path::ValueTarget};
use crate::state::change::{StoreCommitPolicy, StorePatch, StoreTransaction, StoreWriteOrigin};
use crate::widgets::node::{NodeWalkScope, find_node, walk_nodes_mut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitPhase {
    Live,
    Submit,
}

impl AppState {
    pub(super) fn sync_current_step_values_to_store(&mut self) {
        let applied = self.refresh_current_step_bindings_for_phase(CommitPhase::Submit);
        self.emit_store_change_triggers(applied.into_targets());
    }

    pub(super) fn refresh_current_step_bindings(&mut self) {
        let applied = self.refresh_current_step_bindings_for_phase(CommitPhase::Live);
        self.emit_store_change_triggers(applied.into_targets());
    }

    pub(super) fn refresh_current_step_bindings_collect_for_live(&mut self) -> AppliedStorePatch {
        self.refresh_current_step_bindings_for_phase(CommitPhase::Live)
    }

    fn refresh_current_step_bindings_for_phase(&mut self, phase: CommitPhase) -> AppliedStorePatch {
        let mut applied = AppliedStorePatch::default();

        let bootstrap_patch = self.bootstrap_missing_current_step_value_bindings();
        applied.extend(self.apply_store_patch(bootstrap_patch));

        let commit_patch = self.collect_current_step_commit_patch(phase);
        applied.extend(self.apply_store_transaction(commit_patch));

        self.hydrate_current_step_from_store();
        applied.extend(self.apply_current_step_derived_writes());
        self.hydrate_current_step_from_store();

        applied
    }

    pub(super) fn apply_value_change(&mut self, target: impl Into<NodeId>, value: Value) {
        self.apply_system_value_change(
            ValueTarget::node(target.into()),
            value,
            StoreWriteOrigin::System,
        );
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

    fn bootstrap_missing_current_step_value_bindings(&self) -> StorePatch {
        let mut patch = StorePatch::new();
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
            patch.push(
                binding.target,
                value,
                StoreWriteOrigin::DefaultSeed {
                    node_id: binding.node_id,
                },
            );
        }

        patch
    }

    fn collect_current_step_commit_patch(&self, phase: CommitPhase) -> StoreTransaction {
        let mut transaction = StoreTransaction::new();
        let bindings = self
            .flow
            .current_step()
            .binding_plan
            .direct_value_nodes
            .clone();
        let focused_id = self.ui.focus.current_id().map(str::to_string);

        for binding in bindings {
            let should_commit = match phase {
                CommitPhase::Live => {
                    binding.commit_policy == StoreCommitPolicy::Immediate
                        && focused_id.as_deref() == Some(binding.node_id.as_str())
                }
                CommitPhase::Submit => matches!(
                    binding.commit_policy,
                    StoreCommitPolicy::Immediate | StoreCommitPolicy::OnSubmit
                ),
            };
            if !should_commit {
                continue;
            }

            let nodes = self.flow.current_step().nodes.as_slice();
            let Some(value) =
                find_node(nodes, binding.node_id.as_str()).and_then(|node| node.value())
            else {
                continue;
            };
            let origin = match phase {
                CommitPhase::Live => StoreWriteOrigin::UserInput {
                    node_id: binding.node_id,
                },
                CommitPhase::Submit => StoreWriteOrigin::StepSubmit {
                    node_id: binding.node_id,
                },
            };
            transaction.push(binding.target, value, origin);
        }

        transaction
    }

    pub(super) fn emit_store_change_triggers(&mut self, targets: Vec<ValueTarget>) {
        for target in targets {
            crate::task::engine::trigger_store_value_changed_tasks(self, &target);
        }
    }
}
