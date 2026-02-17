use super::AppState;
use crate::core::{NodeId, value::Value, value_path::ValueTarget};
use crate::widgets::node::{NodeWalkScope, find_node_mut, walk_nodes, walk_nodes_mut};
use std::collections::HashMap;

impl AppState {
    pub(super) fn sync_current_step_values_to_store(&mut self) {
        let values = {
            let mut out = Vec::<(String, Value)>::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Persistent,
                &mut |node| {
                    if let Some(value) = node.value() {
                        out.push((node.id().to_string(), value));
                    }
                },
            );
            out
        };
        for (id, value) in values {
            self.apply_value_change(id, value);
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
        let before = self.data.store.get(root.as_str()).cloned();
        self.data.store.set_target(&target, value);
        let updated = self.data.store.get(root.as_str()).cloned();
        if let Some(updated) = updated {
            let changed = before.as_ref().is_none_or(|previous| previous != &updated);
            self.apply_value_to_step(root.as_str(), updated.clone());
            if changed {
                self.trigger_node_value_changed_tasks(root.as_str(), &updated);
            }
        }
    }

    pub(super) fn hydrate_current_step_from_store(&mut self) {
        let values: HashMap<String, Value> = self
            .data
            .store
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect();

        walk_nodes_mut(
            self.flow.current_step_mut().nodes.as_mut_slice(),
            NodeWalkScope::Persistent,
            &mut |node| {
                if let Some(value) = values.get(node.id()) {
                    node.set_value(value.clone());
                }
            },
        );
    }

    fn apply_value_to_step(&mut self, id: &str, value: Value) {
        if let Some(node) = find_node_mut(self.flow.current_step_mut().nodes.as_mut_slice(), id) {
            node.set_value(value);
            if node
                .validate(crate::widgets::traits::ValidationMode::Live)
                .is_ok()
            {
                self.runtime.validation.clear_error(id);
            }
        }
    }
}
