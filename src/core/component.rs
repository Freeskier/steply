use crate::core::binding::BindTarget;
use crate::core::node::Node;
use crate::core::node::NodeId;
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::RenderContext;
use crate::ui::render::RenderLine;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    PassThrough,
    Group,
}

pub trait Component: Send {
    fn base(&self) -> &ComponentBase;
    fn base_mut(&mut self) -> &mut ComponentBase;

    fn id(&self) -> &str {
        &self.base().id
    }

    fn children(&self) -> Option<&[Node]> {
        None
    }

    fn children_mut(&mut self) -> Option<&mut [Node]> {
        None
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::PassThrough
    }

    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine>;

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

    fn handle_key(
        &mut self,
        _code: KeyCode,
        _modifiers: KeyModifiers,
        _ctx: &mut EventContext,
    ) -> bool {
        false
    }

    fn delete_word(&mut self, _ctx: &mut EventContext) -> bool {
        false
    }

    fn delete_word_forward(&mut self, _ctx: &mut EventContext) -> bool {
        false
    }

    fn render_children(&self) -> bool {
        matches!(self.focus_mode(), FocusMode::PassThrough)
    }
}

#[derive(Debug, Clone)]
pub struct ComponentResponse {
    pub handled: bool,
    pub produced: Option<Value>,
    pub changes: Vec<InputChange>,
    pub submit_requested: bool,
}

#[derive(Debug, Clone)]
pub struct EventContext {
    response: ComponentResponse,
}

#[derive(Debug, Clone)]
pub struct InputChange {
    pub id: NodeId,
    pub value: String,
    pub apply: bool,
}

impl ComponentResponse {
    pub fn not_handled() -> Self {
        Self {
            handled: false,
            produced: None,
            changes: Vec::new(),
            submit_requested: false,
        }
    }

    pub fn handled() -> Self {
        Self {
            handled: true,
            produced: None,
            changes: Vec::new(),
            submit_requested: false,
        }
    }

    pub fn produced(value: Value) -> Self {
        Self {
            handled: true,
            produced: Some(value),
            changes: Vec::new(),
            submit_requested: false,
        }
    }

    pub fn submit_requested() -> Self {
        Self {
            handled: true,
            produced: None,
            changes: Vec::new(),
            submit_requested: true,
        }
    }

    pub fn push_change(&mut self, id: impl Into<NodeId>, value: impl Into<String>) {
        self.changes.push(InputChange {
            id: id.into(),
            value: value.into(),
            apply: true,
        });
    }
}

impl EventContext {
    pub fn new() -> Self {
        Self {
            response: ComponentResponse::not_handled(),
        }
    }

    pub fn handled(&mut self) {
        self.response.handled = true;
    }

    pub fn produce(&mut self, value: Value) {
        self.response.handled = true;
        self.response.produced = Some(value);
    }

    pub fn submit(&mut self) {
        self.response.handled = true;
        self.response.submit_requested = true;
    }

    pub fn update_input(&mut self, id: impl Into<NodeId>, value: impl Into<String>) {
        self.response.handled = true;
        self.response.push_change(id, value);
    }

    pub fn record_input(&mut self, id: impl Into<NodeId>, value: impl Into<String>) {
        self.response.handled = true;
        self.response.changes.push(InputChange {
            id: id.into(),
            value: value.into(),
            apply: false,
        });
    }

    pub fn into_response(self, handled: bool) -> ComponentResponse {
        ComponentResponse {
            handled,
            ..self.response
        }
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}
