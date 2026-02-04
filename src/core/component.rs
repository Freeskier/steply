use crate::core::binding::BindTarget;
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::RenderLine;
use crate::ui::theme::Theme;

pub struct ComponentBase {
    pub id: NodeId,
    pub focused: bool,
}

impl ComponentBase {
    pub fn new(id: impl Into<NodeId>) -> Self {
        Self {
            id: id.into(),
            focused: false,
        }
    }
}

pub trait Component: Send {
    fn base(&self) -> &ComponentBase;
    fn base_mut(&mut self) -> &mut ComponentBase;

    fn id(&self) -> &str {
        &self.base().id
    }

    fn node_ids(&self) -> &[NodeId];

    fn nodes(&mut self) -> Vec<(NodeId, Node)>;

    fn render(&self, registry: &NodeRegistry, theme: &Theme) -> Vec<RenderLine>;

    fn is_focused(&self) -> bool {
        self.base().focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.base_mut().focused = focused;
    }

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
