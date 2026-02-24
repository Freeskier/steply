use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::runtime::event::{SystemEvent, ValueChange, WidgetAction};
use crate::task::{TaskSpec, TaskSubscription};
use crate::terminal::{CursorPos, KeyEvent, PointerEvent, PointerSemantic, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::widgets::shared::text_edit;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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

    pub visible_errors: Arc<HashMap<String, String>>,

    pub invalid_hidden: Arc<HashSet<String>>,
    pub completion_menus: Arc<HashMap<String, CompletionMenu>>,
}

impl RenderContext {
    pub fn empty(terminal_size: TerminalSize) -> Self {
        Self {
            focused_id: None,
            terminal_size,
            visible_errors: Arc::new(HashMap::new()),
            invalid_hidden: Arc::new(HashSet::new()),
            completion_menus: Arc::new(HashMap::new()),
        }
    }

    pub fn with_focus(&self, focused_id: Option<String>) -> Self {
        Self {
            focused_id,
            terminal_size: self.terminal_size,
            visible_errors: self.visible_errors.clone(),
            invalid_hidden: self.invalid_hidden.clone(),
            completion_menus: self.completion_menus.clone(),
        }
    }
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
    fn pointer_rows(&self, _ctx: &RenderContext) -> Vec<PointerRowMap> {
        Vec::new()
    }
    fn hints(&self, _ctx: HintContext) -> Vec<HintItem> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerRowMap {
    pub rendered_row: u16,
    pub local_row: u16,
    pub local_col_offset: u16,
    pub local_semantic: PointerSemantic,
}

impl PointerRowMap {
    pub fn new(rendered_row: u16, local_row: u16) -> Self {
        Self {
            rendered_row,
            local_row,
            local_col_offset: 0,
            local_semantic: PointerSemantic::None,
        }
    }

    pub fn with_local_col_offset(mut self, offset: u16) -> Self {
        self.local_col_offset = offset;
        self
    }

    pub fn with_semantic(mut self, semantic: PointerSemantic) -> Self {
        self.local_semantic = semantic;
        self
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
    fn on_pointer(&mut self, _event: PointerEvent) -> InteractionResult {
        InteractionResult::ignored()
    }

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
    fn cursor_visible(&self) -> bool {
        true
    }

    fn value(&self) -> Option<Value> {
        None
    }
    fn set_value(&mut self, _value: Value) {}

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        Vec::new()
    }

    fn task_subscriptions(&self) -> Vec<TaskSubscription> {
        Vec::new()
    }
}

pub trait InteractiveFocusCapability {
    fn focus_mode_cap(&self) -> FocusMode;
}

impl<T> InteractiveFocusCapability for T
where
    T: Interactive + ?Sized,
{
    fn focus_mode_cap(&self) -> FocusMode {
        Interactive::focus_mode(self)
    }
}

pub trait InteractiveOverlayCapability {
    fn overlay_placement_cap(&self) -> Option<OverlayPlacement>;
    fn overlay_open_cap(&mut self, saved_focus_id: Option<String>) -> bool;
    fn overlay_close_cap(&mut self) -> Option<String>;
    fn overlay_mode_cap(&self) -> OverlayMode;
}

impl<T> InteractiveOverlayCapability for T
where
    T: Interactive + ?Sized,
{
    fn overlay_placement_cap(&self) -> Option<OverlayPlacement> {
        Interactive::overlay_placement(self)
    }

    fn overlay_open_cap(&mut self, saved_focus_id: Option<String>) -> bool {
        Interactive::overlay_open(self, saved_focus_id)
    }

    fn overlay_close_cap(&mut self) -> Option<String> {
        Interactive::overlay_close(self)
    }

    fn overlay_mode_cap(&self) -> OverlayMode {
        Interactive::overlay_mode(self)
    }
}

pub trait InteractiveInputCapability {
    fn on_key_cap(&mut self, key: KeyEvent) -> InteractionResult;
    fn on_pointer_cap(&mut self, event: PointerEvent) -> InteractionResult;
    fn on_text_action_cap(&mut self, action: TextAction) -> InteractionResult;
    fn on_text_edited_cap(&mut self);
    fn completion_cap(&mut self) -> Option<CompletionState<'_>>;
}

impl<T> InteractiveInputCapability for T
where
    T: Interactive + ?Sized,
{
    fn on_key_cap(&mut self, key: KeyEvent) -> InteractionResult {
        Interactive::on_key(self, key)
    }

    fn on_pointer_cap(&mut self, event: PointerEvent) -> InteractionResult {
        Interactive::on_pointer(self, event)
    }

    fn on_text_action_cap(&mut self, action: TextAction) -> InteractionResult {
        Interactive::on_text_action(self, action)
    }

    fn on_text_edited_cap(&mut self) {
        Interactive::on_text_edited(self);
    }

    fn completion_cap(&mut self) -> Option<CompletionState<'_>> {
        Interactive::completion(self)
    }
}

pub trait InteractiveRuntimeCapability {
    fn on_system_event_cap(&mut self, event: &SystemEvent) -> InteractionResult;
    fn on_tick_cap(&mut self) -> InteractionResult;
}

impl<T> InteractiveRuntimeCapability for T
where
    T: Interactive + ?Sized,
{
    fn on_system_event_cap(&mut self, event: &SystemEvent) -> InteractionResult {
        Interactive::on_system_event(self, event)
    }

    fn on_tick_cap(&mut self) -> InteractionResult {
        Interactive::on_tick(self)
    }
}

pub trait InteractiveCursorCapability {
    fn cursor_pos_cap(&self) -> Option<CursorPos>;
    fn cursor_visible_cap(&self) -> bool;
}

impl<T> InteractiveCursorCapability for T
where
    T: Interactive + ?Sized,
{
    fn cursor_pos_cap(&self) -> Option<CursorPos> {
        Interactive::cursor_pos(self)
    }

    fn cursor_visible_cap(&self) -> bool {
        Interactive::cursor_visible(self)
    }
}

pub trait InteractiveValueCapability {
    fn value_cap(&self) -> Option<Value>;
    fn set_value_cap(&mut self, value: Value);
}

impl<T> InteractiveValueCapability for T
where
    T: Interactive + ?Sized,
{
    fn value_cap(&self) -> Option<Value> {
        Interactive::value(self)
    }

    fn set_value_cap(&mut self, value: Value) {
        Interactive::set_value(self, value);
    }
}

pub trait InteractiveValidationCapability {
    fn validate_cap(&self, mode: ValidationMode) -> Result<(), String>;
}

impl<T> InteractiveValidationCapability for T
where
    T: Interactive + ?Sized,
{
    fn validate_cap(&self, mode: ValidationMode) -> Result<(), String> {
        Interactive::validate(self, mode)
    }
}

pub trait InteractiveTaskCapability {
    fn task_specs_cap(&self) -> Vec<TaskSpec>;
    fn task_subscriptions_cap(&self) -> Vec<TaskSubscription>;
}

impl<T> InteractiveTaskCapability for T
where
    T: Interactive + ?Sized,
{
    fn task_specs_cap(&self) -> Vec<TaskSpec> {
        Interactive::task_specs(self)
    }

    fn task_subscriptions_cap(&self) -> Vec<TaskSubscription> {
        Interactive::task_subscriptions(self)
    }
}

pub trait InteractiveNode: Drawable + Interactive {}
impl<T> InteractiveNode for T where T: Drawable + Interactive {}

pub trait OutputNode: Drawable {
    fn value(&self) -> Option<Value> {
        None
    }
    fn set_value(&mut self, _value: Value) {}
    fn on_pointer(&mut self, _event: PointerEvent) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn on_tick(&mut self) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn on_system_event(&mut self, _event: &SystemEvent) -> InteractionResult {
        InteractionResult::ignored()
    }
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        Vec::new()
    }

    fn task_subscriptions(&self) -> Vec<TaskSubscription> {
        Vec::new()
    }
}
