use crate::app::event::WidgetEvent;
use crate::domain::value::Value;
use crate::node::Node;
use crate::terminal::terminal::{CursorPos, KeyEvent, TerminalSize};
use crate::ui::span::{Span, SpanLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    None,
    Leaf,
    Group,
    Container,
}

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub focused_id: Option<String>,
    pub terminal_size: TerminalSize,
}

#[derive(Debug, Clone, Default)]
pub struct DrawOutput {
    pub lines: Vec<SpanLine>,
}

impl DrawOutput {
    pub fn plain_lines(lines: Vec<String>) -> Self {
        Self {
            lines: lines
                .into_iter()
                .map(|line| vec![Span::new(line).no_wrap()])
                .collect::<Vec<_>>(),
        }
    }
}

pub trait Drawable: Send {
    fn id(&self) -> &str;
    fn draw(&self, ctx: &RenderContext) -> DrawOutput;
}

#[derive(Debug, Clone, Default)]
pub struct InteractionResult {
    pub handled: bool,
    pub events: Vec<WidgetEvent>,
}

impl InteractionResult {
    pub fn ignored() -> Self {
        Self::default()
    }

    pub fn handled() -> Self {
        Self {
            handled: true,
            events: Vec::new(),
        }
    }

    pub fn with_event(event: WidgetEvent) -> Self {
        Self {
            handled: true,
            events: vec![event],
        }
    }
}

pub trait Interactive: Send {
    fn focus_mode(&self) -> FocusMode;
    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult;
    fn on_event(&mut self, _event: &WidgetEvent) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn on_tick(&mut self) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn cursor_pos(&self) -> Option<CursorPos> {
        None
    }

    fn value(&self) -> Option<Value> {
        None
    }
    fn set_value(&mut self, _value: Value) {}
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    fn children(&self) -> Option<&[Node]> {
        None
    }
    fn children_mut(&mut self) -> Option<&mut [Node]> {
        None
    }
}

pub trait InteractiveNode: Drawable + Interactive {}
impl<T> InteractiveNode for T where T: Drawable + Interactive {}

pub trait RenderNode: Drawable {}
impl<T> RenderNode for T where T: Drawable {}
