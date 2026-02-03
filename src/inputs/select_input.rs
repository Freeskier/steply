use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;

pub struct SelectInput {
    base: InputBase,
    options: Vec<String>,
    selected: usize,
}

impl SelectInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            options,
            selected: 0,
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

    fn current_option(&self) -> Option<&str> {
        self.options.get(self.selected).map(|s| s.as_str())
    }

    fn move_left(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let len = self.options.len();
        self.selected = (self.selected + len - 1) % len;
    }

    fn move_right(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let len = self.options.len();
        self.selected = (self.selected + 1) % len;
    }
}

impl Input for SelectInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        self.current_option().unwrap_or("").to_string()
    }

    fn set_value(&mut self, value: String) {
        if let Some(pos) = self.options.iter().position(|opt| opt == &value) {
            self.selected = pos;
        }
    }

    fn raw_value(&self) -> String {
        self.value()
    }

    fn is_complete(&self) -> bool {
        !self.options.is_empty()
    }

    fn cursor_pos(&self) -> usize {
        0
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Left => {
                self.move_left();
                KeyResult::Handled
            }
            KeyCode::Right => {
                self.move_right();
                KeyResult::Handled
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, _theme: &crate::theme::Theme) -> Vec<Span> {
        let option = self.current_option().unwrap_or("");
        let text = if self.base.focused {
            format!("‹{}›", option)
        } else {
            option.to_string()
        };
        vec![Span::new(text)]
    }

    fn cursor_offset_in_content(&self) -> usize {
        0
    }
}
