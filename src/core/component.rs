use crate::core::binding::BindTarget;
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};

pub enum ComponentItem {
    Node(NodeId),
    Text(String),
    Separator,
    Option {
        cursor: String,
        marker_left: String,
        marker: String,
        marker_right: String,
        text: String,
        active: bool,
        selected: bool,
    },
}

pub trait Component: Send {
    fn id(&self) -> &str;

    fn node_ids(&self) -> &[NodeId];

    fn nodes(&mut self) -> Vec<(NodeId, Node)>;

    fn items(&self, registry: &NodeRegistry) -> Vec<ComponentItem>;

    fn is_focused(&self) -> bool {
        false
    }

    fn set_focused(&mut self, _focused: bool) {}

    fn bind_target(&self) -> Option<BindTarget> {
        None
    }

    fn value(&self) -> Option<Value> {
        None
    }

    fn set_value(&mut self, _value: Value) {}

    fn handle_key(&mut self, _code: KeyCode, _modifiers: KeyModifiers) -> ComponentResponse {
        ComponentResponse::not_handled()
    }
}

#[derive(Debug, Clone)]
pub struct ComponentResponse {
    pub handled: bool,
    pub produced: Option<Value>,
}

impl ComponentResponse {
    pub fn not_handled() -> Self {
        Self {
            handled: false,
            produced: None,
        }
    }

    pub fn handled() -> Self {
        Self {
            handled: true,
            produced: None,
        }
    }

    pub fn produced(value: Value) -> Self {
        Self {
            handled: true,
            produced: Some(value),
        }
    }
}
