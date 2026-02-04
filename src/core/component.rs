use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::terminal::{KeyCode, KeyModifiers};

pub enum ComponentItem {
    Node(NodeId),
    Text(String),
    Separator,
    Option { text: String, active: bool },
}

pub trait Component: Send {
    fn id(&self) -> &str;

    fn node_ids(&self) -> &[NodeId];

    fn nodes(&mut self) -> Vec<(NodeId, Node)>;

    fn items(&self, registry: &NodeRegistry) -> Vec<ComponentItem>;

    fn handle_key(&mut self, _code: KeyCode, _modifiers: KeyModifiers) -> bool {
        false
    }
}
