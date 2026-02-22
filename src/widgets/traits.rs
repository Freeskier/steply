use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::runtime::event::{SystemEvent, ValueChange, WidgetAction};
use crate::terminal::{CursorPos, KeyEvent, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::widgets::inputs::text_edit;
use std::borrow::Cow;
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
pub enum OverlayRenderMode {
    Floating,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayPlacement {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub height: u16,
    pub render_mode: OverlayRenderMode,
}

impl OverlayPlacement {
    pub fn new(row: u16, col: u16, width: u16, height: u16) -> Self {
        Self {
            row,
            col,
            width,
            height,
            render_mode: OverlayRenderMode::Floating,
        }
    }

    pub fn with_render_mode(mut self, render_mode: OverlayRenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }
}












#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Live,
    Submit,
}





#[derive(Debug, Clone)]
pub struct CompletionMenu {
    pub matches: Vec<String>,
    pub selected: usize,
    pub start: usize,
}

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub focused_id: Option<String>,
    pub terminal_size: TerminalSize,

    pub visible_errors: HashMap<String, String>,

    pub invalid_hidden: HashSet<String>,
    pub completion_menus: HashMap<String, CompletionMenu>,
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
                .collect(),
        }
    }
}





pub trait Drawable: Send {
    fn id(&self) -> &str;
    fn label(&self) -> &str {
        ""
    }
    fn draw(&self, ctx: &RenderContext) -> DrawOutput;
    fn hints(&self, _ctx: HintContext) -> Vec<HintItem> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintContext {
    pub focused: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HintGroup {
    Navigation,
    Completion,
    View,
    Action,
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintItem {
    pub key: Cow<'static, str>,
    pub label: Cow<'static, str>,
    pub priority: u8,
    pub group: HintGroup,
}

impl HintItem {
    pub fn new(
        key: impl Into<Cow<'static, str>>,
        label: impl Into<Cow<'static, str>>,
        group: HintGroup,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            priority: 50,
            group,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}





#[derive(Debug, Clone, Default)]
pub struct InteractionResult {
    pub handled: bool,
    pub request_render: bool,
    pub actions: Vec<WidgetAction>,
}

impl InteractionResult {
    pub fn ignored() -> Self {
        Self::default()
    }

    pub fn consumed() -> Self {
        Self {
            handled: true,
            request_render: false,
            actions: Vec::new(),
        }
    }

    pub fn handled() -> Self {
        Self {
            handled: true,
            request_render: true,
            actions: Vec::new(),
        }
    }

    pub fn with_action(action: WidgetAction) -> Self {
        Self {
            handled: true,
            request_render: true,
            actions: vec![action],
        }
    }

    pub fn input_done() -> Self {
        Self::with_action(WidgetAction::InputDone)
    }

    pub fn submit_or_produce(target: Option<&ValueTarget>, value: Value) -> Self {
        if let Some(target) = target {
            return Self::with_action(WidgetAction::ValueChanged {
                change: ValueChange::with_target(target.clone(), value),
            });
        }
        Self::input_done()
    }

    pub fn merge(&mut self, other: Self) {
        self.handled |= other.handled;
        self.request_render |= other.request_render;
        self.actions.extend(other.actions);
    }
}





#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAction {
    DeleteWordLeft,
    DeleteWordRight,
    MoveWordLeft,
    MoveWordRight,
}

pub struct TextEditState<'a> {
    pub value: &'a mut String,
    pub cursor: &'a mut usize,
}

pub struct CompletionState<'a> {
    pub value: &'a mut String,
    pub cursor: &'a mut usize,
    pub candidates: &'a [String],

    pub prefix_start: Option<usize>,
}

impl TextAction {
    pub(crate) fn apply(self, state: &mut TextEditState<'_>) -> bool {
        match self {
            Self::DeleteWordLeft => text_edit::delete_word_left(state.value, state.cursor),
            Self::DeleteWordRight => text_edit::delete_word_right(state.value, state.cursor),
            Self::MoveWordLeft => text_edit::move_word_left(state.cursor, state.value),
            Self::MoveWordRight => text_edit::move_word_right(state.cursor, state.value),
        }
    }
}





pub trait Interactive: Send {
    fn focus_mode(&self) -> FocusMode;



    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        None
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

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        None
    }
    fn on_text_edited(&mut self) {}
    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let Some(mut state) = self.text_editing() else {
            return InteractionResult::ignored();
        };
        if action.apply(&mut state) {
            self.on_text_edited();
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        None
    }

    fn on_system_event(&mut self, _event: &SystemEvent) -> InteractionResult {
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









    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }
}





pub trait InteractiveNode: Drawable + Interactive {}
impl<T> InteractiveNode for T where T: Drawable + Interactive {}





pub trait OutputNode: Drawable {
    fn value(&self) -> Option<Value> {
        None
    }
    fn set_value(&mut self, _value: Value) {}
    fn on_tick(&mut self) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn on_system_event(&mut self, _event: &SystemEvent) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}
