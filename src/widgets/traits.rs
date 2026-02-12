use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyEvent, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::widgets::node::Node;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    None,
    Leaf,
    Group,
    Container,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    Exclusive,
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayPlacement {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub height: u16,
}

impl OverlayPlacement {
    pub fn new(row: u16, col: u16, width: u16, height: u16) -> Self {
        Self {
            row,
            col,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub focused_id: Option<String>,
    pub terminal_size: TerminalSize,
    pub visible_errors: HashMap<String, String>,
    pub invalid_hidden: HashSet<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAction {
    DeleteWordLeft,
    DeleteWordRight,
}

pub trait Interactive: Send {
    fn focus_mode(&self) -> FocusMode;

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        None
    }
    fn overlay_is_visible(&self) -> bool {
        false
    }
    fn overlay_open(&mut self, _saved_focus_id: Option<String>) -> bool {
        false
    }
    fn overlay_close(&mut self) -> Option<String> {
        None
    }
    fn overlay_mode(&self) -> OverlayMode {
        OverlayMode::Exclusive
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult;
    fn on_text_action(&mut self, _action: TextAction) -> InteractionResult {
        InteractionResult::ignored()
    }
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
