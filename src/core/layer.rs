use crate::core::event_queue::AppEvent;
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;

pub trait Layer {
    fn id(&self) -> &str;

    fn label(&self) -> &str;

    fn hint(&self) -> Option<&str>;

    fn node_ids(&self) -> &[NodeId];

    fn nodes(&mut self) -> Vec<(NodeId, Node)>;

    fn emit_close_events(&mut self, _registry: &NodeRegistry, _emit: &mut dyn FnMut(AppEvent)) {}
}

pub struct ActiveLayer {
    pub layer: Box<dyn Layer>,
    pub saved_focus_id: Option<NodeId>,
}

impl ActiveLayer {
    pub fn new(layer: Box<dyn Layer>, saved_focus_id: Option<NodeId>) -> Self {
        Self {
            layer,
            saved_focus_id,
        }
    }

    pub fn label(&self) -> &str {
        self.layer.label()
    }

    pub fn hint(&self) -> Option<&str> {
        self.layer.hint()
    }

    pub fn node_ids(&self) -> &[NodeId] {
        self.layer.node_ids()
    }

    pub fn first_input_id(&self) -> Option<&NodeId> {
        self.layer.node_ids().first()
    }
}
