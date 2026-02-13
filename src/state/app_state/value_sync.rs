use super::AppState;
use crate::core::value::Value;
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
            self.set_value_by_id(&id, value);
        }
    }

    pub(super) fn set_value_by_id(&mut self, id: &str, value: Value) {
        self.write_value_direct(id, value);
    }

    fn write_value_direct(&mut self, id: &str, value: Value) {
        let changed = self
            .data
            .store
            .get(id)
            .is_none_or(|current| current != &value);
        self.data.store.set(id.to_string(), value.clone());
        self.apply_value_to_step(id, value);
        if changed && let Some(updated) = self.data.store.get(id).cloned() {
            self.trigger_node_value_changed_tasks(id, &updated);
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
            if node.validate_live().is_ok() {
                self.runtime.validation.clear_error(id);
            }
        }
    }
}
