use super::text_edit;
use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::Style;
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, TextEditState,
};
use crate::widgets::validators::Validator;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRenderMode {
    Stars,
    Hidden,
}

pub struct PasswordInput {
    base: InputBase,
    value: String,
    cursor: usize,
    submit_target: Option<String>,
    validators: Vec<Validator>,
    render_mode: PasswordRenderMode,
}

impl PasswordInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            value: String::new(),
            cursor: 0,
            submit_target: None,
            validators: Vec::new(),
            render_mode: PasswordRenderMode::Stars,
        }
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_render_mode(mut self, render_mode: PasswordRenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }

    fn masked_value(&self) -> String {
        let len = text_edit::char_count(&self.value);
        match self.render_mode {
            PasswordRenderMode::Stars => "*".repeat(len),
            PasswordRenderMode::Hidden => " ".repeat(len),
        }
    }
}

impl Drawable for PasswordInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line = self.base.line_state(ctx);

        let masked = self.masked_value();

        DrawOutput {
            lines: vec![vec![
                Span::new(line.prefix).no_wrap(),
                Span::styled(masked, Style::default()).no_wrap(),
            ]],
        }
    }
}

impl Interactive for PasswordInput {
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
            KeyCode::Left => {
                if text_edit::move_left(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if text_edit::move_right(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter => InteractionResult::submit_or_produce(
                self.submit_target.as_deref(),
                Value::Text(self.value.clone()),
            ),
            _ => InteractionResult::ignored(),
        }
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        Some(TextEditState {
            value: &mut self.value,
            cursor: &mut self.cursor,
        })
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.value.clone()))
    }

    fn set_value(&mut self, value: Value) {
        if let Value::Text(v) = value {
            self.value = v;
            self.cursor = text_edit::char_count(&self.value);
        }
    }

    fn validate_submit(&self) -> Result<(), String> {
        for validator in &self.validators {
            validator(&self.value)?;
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let prefix = format!("{} {}: ", self.base.focus_marker(true), self.base.label());
        Some(CursorPos {
            col: (UnicodeWidthStr::width(prefix.as_str())
                + text_edit::clamp_cursor(self.cursor, &self.value)) as u16,
            row: 0,
        })
    }
}
