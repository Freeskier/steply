use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::style::{Color, Style};
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;

pub struct CheckboxInput {
    base: InputBase,
    checked: bool,
}

impl CheckboxInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            checked: false,
        }
    }

    pub fn with_min_width(mut self, width: usize) -> Self {
        self.base = self.base.with_min_width(width);
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.base = self.base.with_validator(validator);
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.base = self.base.with_placeholder(placeholder);
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    fn toggle(&mut self) {
        self.checked = !self.checked;
        self.base.error = None;
    }
}

impl Input for CheckboxInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        if self.checked { "true" } else { "false" }.to_string()
    }

    fn set_value(&mut self, value: String) {
        self.checked = matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes");
    }

    fn raw_value(&self) -> String {
        self.value()
    }

    fn is_complete(&self) -> bool {
        true
    }

    fn cursor_pos(&self) -> usize {
        0
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(' ') => {
                self.toggle();
                KeyResult::Handled
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, _theme: &crate::theme::Theme) -> Vec<Span> {
        let (symbol, color) = if self.checked {
            ("✓", Color::Green)
        } else {
            ("✗", Color::Red)
        };
        let style = Style::new().with_color(color);
        vec![Span::new(symbol).with_style(style)]
    }

    fn cursor_offset_in_content(&self) -> usize {
        0
    }
}
