use crate::core::event_queue::AppEvent;
use crate::core::layer::Layer;
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::text_input::TextInput;
use std::mem;

pub struct OverlayState {
    id: String,
    label: String,
    hint: Option<String>,
    node_ids: Vec<NodeId>,
    nodes: Vec<(NodeId, Node)>,
}

impl OverlayState {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        nodes: Vec<(NodeId, Node)>,
    ) -> Self {
        let node_ids = nodes.iter().map(|(id, _)| id.clone()).collect();
        Self {
            id: id.into(),
            label: label.into(),
            hint: None,
            node_ids,
            nodes,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn demo() -> Self {
        let input_id = "overlay_query".to_string();
        let nodes = vec![(input_id.clone(), Node::input(TextInput::new(&input_id, "Search")))];
        Self::new("overlay_demo", "Overlay demo: type, Esc to close", nodes)
    }
}

impl Layer for OverlayState {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    fn node_ids(&self) -> &[NodeId] {
        &self.node_ids
    }

    fn nodes(&mut self) -> Vec<(NodeId, Node)> {
        mem::take(&mut self.nodes)
    }

    fn emit_close_events(&mut self, registry: &NodeRegistry, emit: &mut dyn FnMut(AppEvent)) {
        let Some(id) = self.node_ids.first() else {
            return;
        };

        let Some(input) = registry.get_input(id) else {
            return;
        };

        let value = input.value();
        if value.is_empty() {
            return;
        }

        emit(AppEvent::LayerResult {
            layer_id: self.id.clone(),
            value,
            target_id: None,
        });
    }
}
