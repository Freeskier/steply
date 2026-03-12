use super::AppState;
use super::transaction::AppliedStorePatch;
use crate::state::change::{StorePatch, StoreWriteOrigin};
use crate::widgets::node::find_node;

impl AppState {
    pub(super) fn apply_current_step_derived_writes(&mut self) -> AppliedStorePatch {
        let writers = self
            .flow
            .current_step()
            .binding_plan
            .derived_writers
            .clone();
        let mut applied = AppliedStorePatch::default();

        for writer in writers {
            let nodes = self.flow.current_step().nodes.as_slice();
            let Some(changes) =
                find_node(nodes, writer.node_id.as_str()).map(|node| node.write_changes())
            else {
                continue;
            };
            let mut patch = StorePatch::new();
            for change in changes {
                patch.push(
                    change.target,
                    change.value,
                    StoreWriteOrigin::Derived {
                        node_id: writer.node_id.clone(),
                    },
                );
            }
            let node_applied = self.apply_store_patch(patch);
            let changed = !node_applied.is_empty();
            applied.extend(node_applied);
            if changed {
                self.hydrate_current_step_from_store();
            }
        }

        applied
    }
}
