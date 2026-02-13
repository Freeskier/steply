use super::text_edit;
use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Display mode for a text input field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextMode {
    /// Plain visible text with optional completion support.
    #[default]
    Plain,
    /// Value is masked with `*` characters. Word-deletion is disabled
    /// (Ctrl+W / Alt+D) since the cursor points at a placeholder, not a real
    /// word boundary.
    Password,
    /// Value is fully hidden (displayed as spaces). The cursor does not move —
    /// it is always shown at the start of the input area.
    Secret,
}

pub struct TextInput {
    base: WidgetBase,
    value: String,
    cursor: usize,
    mode: TextMode,
    submit_target: Option<String>,
    validators: Vec<Validator>,
    completion_items: Vec<String>,
}

impl TextInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            value: String::new(),
            cursor: 0,
            mode: TextMode::Plain,
            submit_target: None,
            validators: Vec::new(),
            completion_items: Vec::new(),
        }
    }

    pub fn with_mode(mut self, mode: TextMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_completion_items(mut self, items: Vec<String>) -> Self {
        self.completion_items = items;
        self
    }

    pub fn set_completion_items(&mut self, items: Vec<String>) {
        self.completion_items = items;
    }

    pub fn completion_items_mut(&mut self) -> &mut Vec<String> {
        &mut self.completion_items
    }

    fn display_value(&self) -> String {
        let len = text_edit::char_count(&self.value);
        match self.mode {
            TextMode::Plain => self.value.clone(),
            TextMode::Password => "*".repeat(len),
            TextMode::Secret => " ".repeat(len),
        }
    }
}

impl Drawable for TextInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let prefix = self.base.input_prefix(ctx);
        let focused = self.base.is_focused(ctx);

        let mut first_line = vec![
            Span::new(prefix).no_wrap(),
            Span::styled(self.display_value(), Style::default()).no_wrap(),
        ];

        // Completion ghost text — only in Plain mode
        if self.mode == TextMode::Plain
            && focused
            && let Some(menu) = ctx.completion_menus.get(self.base.id())
            && let Some(selected) = menu.matches.get(menu.selected)
            && let Some(suffix) = completion_suffix(selected, &self.value, self.cursor)
            && !suffix.is_empty()
        {
            first_line.push(Span::styled(suffix, Style::new().color(Color::DarkGrey)).no_wrap());
        }

        DrawOutput {
            lines: vec![first_line],
        }
    }
}

impl Interactive for TextInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(ch) => {
                text_edit::insert_char(&mut self.value, &mut self.cursor, ch);
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if text_edit::backspace_char(&mut self.value, &mut self.cursor) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Delete => {
                if text_edit::delete_char(&mut self.value, &mut self.cursor) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            // Cursor movement — disabled in Secret mode
            KeyCode::Left if self.mode != TextMode::Secret => {
                if text_edit::move_left(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right if self.mode != TextMode::Secret => {
                if text_edit::move_right(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Home if self.mode != TextMode::Secret => {
                self.cursor = 0;
                InteractionResult::handled()
            }
            KeyCode::End if self.mode != TextMode::Secret => {
                self.cursor = text_edit::char_count(&self.value);
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::submit_or_produce(
                self.submit_target.as_deref(),
                Value::Text(self.value.clone()),
            ),
            _ => InteractionResult::ignored(),
        }
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        // Password/Secret: word-deletion is disabled. Return None so that
        // on_text_action (Ctrl+W / Alt+D) is a no-op for these modes.
        if self.mode != TextMode::Plain {
            return None;
        }
        Some(TextEditState {
            value: &mut self.value,
            cursor: &mut self.cursor,
        })
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        // Completion only makes sense for plain text
        if self.mode != TextMode::Plain {
            return None;
        }
        Some(CompletionState {
            value: &mut self.value,
            cursor: &mut self.cursor,
            candidates: self.completion_items.as_slice(),
        })
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueChanged { change } if change.target.as_str() == self.base.id() => {
                if let Value::Text(v) = &change.value {
                    self.value = v.clone();
                    self.cursor = text_edit::char_count(&self.value);
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.value.clone()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(v) = value.to_text_scalar() {
            self.value = v;
            self.cursor = text_edit::char_count(&self.value);
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &self.value)
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let prefix = self.base.input_prefix_focused();
        let prefix_width = UnicodeWidthStr::width(prefix.as_str()) as u16;

        let col = match self.mode {
            // Secret: cursor always at the start of the value area
            TextMode::Secret => prefix_width,
            // Password: cursor tracks char position but each char is 1-wide (*)
            TextMode::Password => {
                prefix_width + text_edit::clamp_cursor(self.cursor, &self.value) as u16
            }
            // Plain: cursor tracks unicode width
            TextMode::Plain => {
                let value_width: usize = self
                    .value
                    .chars()
                    .take(text_edit::clamp_cursor(self.cursor, &self.value))
                    .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
                    .sum();
                prefix_width + value_width as u16
            }
        };

        Some(CursorPos { col, row: 0 })
    }
}

fn completion_suffix(selected: &str, value: &str, cursor: usize) -> Option<String> {
    let (_, token) = text_edit::completion_prefix(value, cursor)?;
    if token.is_empty() {
        return None;
    }
    if !selected.to_lowercase().starts_with(&token.to_lowercase()) {
        return None;
    }
    let token_len = token.chars().count();
    Some(selected.chars().skip(token_len).collect())
}
