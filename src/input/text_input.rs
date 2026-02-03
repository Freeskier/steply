use crate::input::{Input, InputBase, InputCaps, KeyResult, NodeId};
use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

pub struct TextInput {
    base: InputBase,
    value: String,
    cursor_pos: usize,
}

impl TextInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            value: String::new(),
            cursor_pos: 0,
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

    fn handle_char(&mut self, ch: char) {
        let char_indices: Vec<usize> = self.value.char_indices().map(|(i, _)| i).collect();
        let byte_pos = if self.cursor_pos >= char_indices.len() {
            self.value.len()
        } else {
            char_indices[self.cursor_pos]
        };
        self.value.insert(byte_pos, ch);
        self.cursor_pos += 1;
        self.base.error = None;
    }

    fn handle_backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let char_indices: Vec<usize> = self.value.char_indices().map(|(i, _)| i).collect();
        let byte_pos = char_indices[self.cursor_pos - 1];
        self.value.remove(byte_pos);
        self.cursor_pos -= 1;
        self.base.error = None;
    }

    fn move_left(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            true
        } else {
            false
        }
    }

    fn move_right(&mut self) -> bool {
        if self.cursor_pos < self.value.chars().count() {
            self.cursor_pos += 1;
            true
        } else {
            false
        }
    }

    fn is_separator(ch: char) -> bool {
        ch.is_whitespace() || matches!(ch, '.' | '/' | ',' | '-' | '@')
    }

    fn move_word_left(&mut self) -> bool {
        if self.cursor_pos == 0 {
            return false;
        }

        let chars: Vec<char> = self.value.chars().collect();
        let mut pos = self.cursor_pos;

        while pos > 0 && chars.get(pos - 1).is_some_and(|c| Self::is_separator(*c)) {
            pos -= 1;
        }

        while pos > 0 && chars.get(pos - 1).is_some_and(|c| !Self::is_separator(*c)) {
            pos -= 1;
        }

        self.cursor_pos = pos;
        true
    }

    fn move_word_right(&mut self) -> bool {
        let chars: Vec<char> = self.value.chars().collect();
        let mut pos = self.cursor_pos;

        while pos < chars.len() && chars.get(pos).is_some_and(|c| Self::is_separator(*c)) {
            pos += 1;
        }

        while pos < chars.len() && chars.get(pos).is_some_and(|c| !Self::is_separator(*c)) {
            pos += 1;
        }

        if pos == self.cursor_pos {
            false
        } else {
            self.cursor_pos = pos;
            true
        }
    }

    fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_end(&mut self) {
        self.cursor_pos = self.value.chars().count();
    }

    fn delete_word_impl(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let mut chars: Vec<char> = self.value.chars().collect();
        let mut pos = self.cursor_pos;

        while pos > 0 && chars.get(pos - 1).is_some_and(|c| Self::is_separator(*c)) {
            chars.remove(pos - 1);
            pos -= 1;
        }

        while pos > 0 && chars.get(pos - 1).is_some_and(|c| !Self::is_separator(*c)) {
            chars.remove(pos - 1);
            pos -= 1;
        }

        self.value = chars.into_iter().collect();
        self.cursor_pos = pos;
        self.base.error = None;
    }

    fn delete_word_forward_impl(&mut self) {
        let mut chars: Vec<char> = self.value.chars().collect();
        let pos = self.cursor_pos;

        while pos < chars.len() && chars.get(pos).is_some_and(|c| Self::is_separator(*c)) {
            chars.remove(pos);
        }

        while pos < chars.len() && chars.get(pos).is_some_and(|c| !Self::is_separator(*c)) {
            chars.remove(pos);
        }

        self.value = chars.into_iter().collect();
        self.base.error = None;
    }
}

impl Input for TextInput {
    fn id(&self) -> &NodeId {
        &self.base.id
    }

    fn label(&self) -> &str {
        &self.base.label
    }

    fn value(&self) -> String {
        self.value.clone()
    }

    fn set_value(&mut self, value: String) {
        self.cursor_pos = value.chars().count();
        self.value = value;
    }

    fn raw_value(&self) -> String {
        self.value.clone()
    }

    fn is_complete(&self) -> bool {
        true
    }

    fn is_focused(&self) -> bool {
        self.base.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        if !focused {
            self.base.error = None;
        }
    }

    fn error(&self) -> Option<&str> {
        self.base.error.as_deref()
    }

    fn set_error(&mut self, error: Option<String>) {
        self.base.error = error;
    }

    fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    fn min_width(&self) -> usize {
        self.base.min_width
    }

    fn validators(&self) -> &[Validator] {
        &self.base.validators
    }

    fn capabilities(&self) -> InputCaps {
        InputCaps {
            capture_ctrl_backspace: true,
            capture_ctrl_delete: true,
            capture_ctrl_left: true,
            capture_ctrl_right: true,
            ..InputCaps::default()
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(ch) => {
                self.handle_char(ch);
                KeyResult::Handled
            }
            KeyCode::Backspace => {
                self.handle_backspace();
                KeyResult::Handled
            }
            KeyCode::Left => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_word_left();
                } else {
                    self.move_left();
                }
                KeyResult::Handled
            }
            KeyCode::Right => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_word_right();
                } else {
                    self.move_right();
                }
                KeyResult::Handled
            }
            KeyCode::Home => {
                self.move_home();
                KeyResult::Handled
            }
            KeyCode::End => {
                self.move_end();
                KeyResult::Handled
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self) -> Vec<Span> {
        vec![Span::new(&self.value)]
    }

    fn cursor_offset_in_content(&self) -> usize {
        self.value
            .chars()
            .take(self.cursor_pos)
            .map(|c| c.to_string().width())
            .sum()
    }

    fn delete_word(&mut self) {
        self.delete_word_impl();
    }

    fn delete_word_forward(&mut self) {
        self.delete_word_forward_impl();
    }
}
