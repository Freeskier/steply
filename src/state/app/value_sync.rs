use super::AppState;
use crate::core::{NodeId, value::Value};
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
        self.write_value_direct(target.into(), value);
    }

    fn write_value_direct(&mut self, id: NodeId, value: Value) {
        let changed = self
            .data
            .store
            .get(id.as_str())
            .is_none_or(|current| current != &value);
        self.data.store.set(id.clone(), value.clone());
        self.apply_value_to_step(id.as_str(), value);
        if changed && let Some(updated) = self.data.store.get(id.as_str()).cloned() {
            self.trigger_node_value_changed_tasks(id.as_str(), &updated);
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
