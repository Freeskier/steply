use super::text_edit;
use crate::inputs::{Input, InputBase, KeyResult};
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

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.base = self.base.with_placeholder(placeholder);
        self
    }

    pub(crate) fn base_ref(&self) -> &InputBase {
        &self.base
    }

    pub(crate) fn base_mut_ref(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn handle_char(&mut self, ch: char) {
        text_edit::insert_char(&mut self.value, &mut self.cursor_pos, ch);
        self.base.error = None;
    }

    fn handle_backspace(&mut self) {
        if text_edit::backspace_char(&mut self.value, &mut self.cursor_pos) {
            self.base.error = None;
        }
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
        if self.cursor_pos < text_edit::char_count(&self.value) {
            self.cursor_pos += 1;
            true
        } else {
            false
        }
    }

    fn move_word_left(&mut self) -> bool {
        text_edit::move_word_left(
            &self.value,
            &mut self.cursor_pos,
            text_edit::default_word_separator,
        )
    }

    fn move_word_right(&mut self) -> bool {
        text_edit::move_word_right(
            &self.value,
            &mut self.cursor_pos,
            text_edit::default_word_separator,
        )
    }

    fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_end(&mut self) {
        self.cursor_pos = text_edit::char_count(&self.value);
    }

    fn delete_word_impl(&mut self) {
        if text_edit::delete_word_left(
            &mut self.value,
            &mut self.cursor_pos,
            text_edit::default_word_separator,
        ) {
            self.base.error = None;
        }
    }

    fn delete_word_forward_impl(&mut self) {
        if text_edit::delete_word_right(
            &mut self.value,
            &mut self.cursor_pos,
            text_edit::default_word_separator,
        ) {
            self.base.error = None;
        }
    }
}

impl Input for TextInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        self.value.clone()
    }

    fn set_value(&mut self, value: String) {
        self.cursor_pos = text_edit::char_count(&value);
        self.value = value;
    }

    fn raw_value(&self) -> String {
        self.value.clone()
    }

    fn is_complete(&self) -> bool {
        true
    }

    fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(ch) => {
                self.handle_char(ch);
                KeyResult::Handled
            }
            KeyCode::Backspace => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    KeyResult::NotHandled
                } else {
                    self.handle_backspace();
                    KeyResult::Handled
                }
            }
            KeyCode::Delete => KeyResult::NotHandled,
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

    fn render_content(&self, _theme: &crate::theme::Theme) -> Vec<Span> {
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
