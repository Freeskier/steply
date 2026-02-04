use crate::core::node::{Node, NodeId};

/// A layer is a UI element that renders on top of the current step.
/// Examples: overlay, modal, command palette, etc.
pub trait Layer {
    /// Unique identifier for this layer
    fn id(&self) -> &str;

    /// Label shown at the top of the layer
    fn label(&self) -> &str;

    /// Optional hint shown below the label
    fn hint(&self) -> Option<&str>;

    /// Node IDs that belong to this layer
    fn node_ids(&self) -> &[NodeId];

    /// Nodes to register when layer opens
    fn nodes(&mut self) -> Vec<(NodeId, Node)>;
}

/// State for an active layer, including saved focus
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
