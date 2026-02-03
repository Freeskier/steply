use crate::input::{Input, InputCaps, KeyResult, NodeId};
use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::text_input::TextInput;
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRender {
    Stars,
    Hidden,
}

pub struct PasswordInput {
    inner: TextInput,
    render_mode: PasswordRender,
}

impl PasswordInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            inner: TextInput::new(id, label),
            render_mode: PasswordRender::Stars,
        }
    }

    pub fn with_min_width(mut self, width: usize) -> Self {
        self.inner = self.inner.with_min_width(width);
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.inner = self.inner.with_validator(validator);
        self
    }

    pub fn with_render_mode(mut self, mode: PasswordRender) -> Self {
        self.render_mode = mode;
        self
    }

    fn raw_len(&self) -> usize {
        self.inner.raw_value().chars().count()
    }
}

impl Input for PasswordInput {
    fn id(&self) -> &NodeId {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn value(&self) -> String {
        self.inner.value()
    }

    fn set_value(&mut self, value: String) {
        self.inner.set_value(value);
    }

    fn raw_value(&self) -> String {
        self.inner.raw_value()
    }

    fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    fn set_focused(&mut self, focused: bool) {
        self.inner.set_focused(focused);
    }

    fn error(&self) -> Option<&str> {
        self.inner.error()
    }

    fn set_error(&mut self, error: Option<String>) {
        self.inner.set_error(error);
    }

    fn cursor_pos(&self) -> usize {
        self.inner.cursor_pos()
    }

    fn min_width(&self) -> usize {
        self.inner.min_width()
    }

    fn validators(&self) -> &[Validator] {
        self.inner.validators()
    }

    fn capabilities(&self) -> InputCaps {
        self.inner.capabilities()
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult {
        self.inner.handle_key(code, modifiers)
    }

    fn render_content(&self) -> Vec<Span> {
        let text = match self.render_mode {
            PasswordRender::Stars => "*".repeat(self.raw_len()),
            PasswordRender::Hidden => " ".repeat(self.raw_len()),
        };
        vec![Span::new(text)]
    }

    fn cursor_offset_in_content(&self) -> usize {
        let content = match self.render_mode {
            PasswordRender::Stars => "*".repeat(self.raw_len()),
            PasswordRender::Hidden => " ".repeat(self.raw_len()),
        };
        content
            .chars()
            .take(self.inner.cursor_pos())
            .map(|c| c.to_string().width())
            .sum()
    }

    fn delete_word(&mut self) {
        self.inner.delete_word();
    }

    fn delete_word_forward(&mut self) {
        self.inner.delete_word_forward();
    }
}
