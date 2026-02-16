use crate::core::value::Value;
use crate::runtime::event::{SystemEvent, ValueChange, WidgetAction};
use crate::terminal::{CursorPos, KeyEvent, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::widgets::inputs::text_edit;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Focus & overlay modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    /// Node does not participate in focus cycling.
    None,
    /// A single focusable leaf (text input, button, checkbox, …).
    Leaf,
    /// A component that manages focus internally among its children.
    Group,
    /// A container that owns persistent children but defers focus to them.
    Container,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    /// Overlay blocks all other focus/input (modal).
    Exclusive,
    /// Overlay shares focus with the base layer.
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

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Controls how strictly a widget validates its current value.
///
/// - `Live`   — called on every keystroke; partial / in-progress input is
///              acceptable (e.g. a masked date field while the user is still
///              typing).
/// - `Submit` — called when the user presses Enter or the step advances;
///              the value must be complete and valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Live,
    Submit,
}

// ---------------------------------------------------------------------------
// Render context & output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CompletionMenu {
    pub matches: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub focused_id: Option<String>,
    pub terminal_size: TerminalSize,
    /// Nodes whose validation error should be shown inline.
    pub visible_errors: HashMap<String, String>,
    /// Nodes that failed validation but the error is not yet revealed.
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

// ---------------------------------------------------------------------------
// Drawable — every node can draw itself
// ---------------------------------------------------------------------------

pub trait Drawable: Send {
    fn id(&self) -> &str;
    fn label(&self) -> &str {
        ""
    }
    fn draw(&self, ctx: &RenderContext) -> DrawOutput;
}

// ---------------------------------------------------------------------------
// InteractionResult
// ---------------------------------------------------------------------------

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

    pub fn submit_or_produce(target: Option<&str>, value: Value) -> Self {
        if let Some(target) = target {
            return Self::with_action(WidgetAction::ValueChanged {
                change: ValueChange::new(target, value),
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

// ---------------------------------------------------------------------------
// TextAction & TextEditState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAction {
    DeleteWordLeft,
    DeleteWordRight,
}

pub struct TextEditState<'a> {
    pub value: &'a mut String,
    pub cursor: &'a mut usize,
}

pub struct CompletionState<'a> {
    pub value: &'a mut String,
    pub cursor: &'a mut usize,
    pub candidates: &'a [String],
}

impl TextAction {
    pub(crate) fn apply(self, state: &mut TextEditState<'_>) -> bool {
        match self {
            Self::DeleteWordLeft => text_edit::delete_word_left(state.value, state.cursor),
            Self::DeleteWordRight => text_edit::delete_word_right(state.value, state.cursor),
        }
    }
}

// ---------------------------------------------------------------------------
// Interactive — input nodes
// ---------------------------------------------------------------------------

pub trait Interactive: Send {
    fn focus_mode(&self) -> FocusMode;

    // --- overlay lifecycle (optional) ---

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

    // --- input handling ---

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

    // --- value ---

    fn value(&self) -> Option<Value> {
        None
    }
    fn set_value(&mut self, _value: Value) {}

    // --- validation ---

    /// Validate the current value.
    ///
    /// `Live` mode is called on every keystroke; partial input is acceptable.
    /// `Submit` mode is called on step submission; the value must be complete.
    ///
    /// Most widgets ignore `mode` and apply the same rules regardless.
    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InteractiveNode — combined bound used in Node
// ---------------------------------------------------------------------------

pub trait InteractiveNode: Drawable + Interactive {}
impl<T> InteractiveNode for T where T: Drawable + Interactive {}

// ---------------------------------------------------------------------------
// OutputNode — output nodes
// ---------------------------------------------------------------------------

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
