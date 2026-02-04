use crate::core::binding::BindTarget;
use crate::core::node::{Node, NodeId};
use crate::core::value::Value;
use crate::inputs::Input;
use indexmap::IndexMap;
use std::collections::HashSet;

pub struct NodeRegistry {
    nodes: IndexMap<NodeId, Node>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
        }
    }

    pub fn insert(&mut self, id: impl Into<NodeId>, node: Node) {
        self.nodes.insert(id.into(), node);
    }

    pub fn get(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Node> {
        self.nodes.shift_remove(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&NodeId, &mut Node)> {
        self.nodes.iter_mut()
    }

    pub fn nodes_for_step<'a>(
        &'a self,
        node_ids: &'a [NodeId],
    ) -> impl Iterator<Item = (&'a NodeId, &'a Node)> {
        node_ids
            .iter()
            .filter_map(|id| self.get(id).map(|n| (id, n)))
    }

    pub fn nodes_for_step_mut<'a>(
        &'a mut self,
        node_ids: &[NodeId],
    ) -> Vec<(&'a NodeId, &'a mut Node)> {
        let ids: HashSet<&NodeId> = node_ids.iter().collect();
        self.nodes
            .iter_mut()
            .filter(|(id, _)| ids.contains(id))
            .collect()
    }

    pub fn get_input(&self, id: &str) -> Option<&dyn Input> {
        self.get(id).and_then(|n| n.as_input())
    }

    pub fn get_input_mut(&mut self, id: &str) -> Option<&mut dyn Input> {
        self.get_mut(id).and_then(|n| n.as_input_mut())
    }

    pub fn get_component(&self, id: &str) -> Option<&dyn crate::core::component::Component> {
        self.get(id).and_then(|n| n.as_component())
    }

    pub fn get_component_mut(
        &mut self,
        id: &str,
    ) -> Option<&mut dyn crate::core::component::Component> {
        self.get_mut(id).and_then(|n| n.as_component_mut())
    }

    pub fn get_value(&self, target: &BindTarget) -> Option<Value> {
        match target {
            BindTarget::Input(id) => self.get_input(id).map(|input| input.value_typed()),
            BindTarget::Component(id) => self.get_component(id).and_then(|c| c.value()),
        }
    }

    pub fn set_value(&mut self, target: &BindTarget, value: Value) {
        match target {
            BindTarget::Input(id) => {
                if let Some(input) = self.get_input_mut(id) {
                    input.set_value_typed(value);
                }
            }
            BindTarget::Component(id) => {
                if let Some(component) = self.get_component_mut(id) {
                    component.set_value(value);
                }
            }
        }
    }

    pub fn input_ids_for_step<'a>(&'a self, node_ids: &'a [NodeId]) -> Vec<&'a NodeId> {
        node_ids
            .iter()
            .filter(|id| self.get(id).map(|n| n.is_input()).unwrap_or(false))
            .collect()
    }

    pub fn input_ids_for_step_owned(&self, node_ids: &[NodeId]) -> Vec<NodeId> {
        self.input_ids_for_step(node_ids)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn extend(&mut self, nodes: impl IntoIterator<Item = (NodeId, Node)>) {
        self.nodes.extend(nodes);
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
