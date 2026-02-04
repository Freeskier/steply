use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::style::{Color, Style};
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

pub struct ButtonInput {
    base: InputBase,
    text: String,
    clicks: i64,
}

impl ButtonInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            base: InputBase::new(id, label.clone()),
            text: label,
            clicks: 0,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
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

    fn click(&mut self) {
        self.clicks = self.clicks.saturating_add(1);
        self.base.error = None;
    }
}

impl Input for ButtonInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        self.clicks.to_string()
    }

    fn set_value(&mut self, value: String) {
        self.clicks = value.parse::<i64>().unwrap_or(0);
    }

    fn value_typed(&self) -> crate::value::Value {
        crate::value::Value::Number(self.clicks)
    }

    fn set_value_typed(&mut self, value: crate::value::Value) {
        match value {
            crate::value::Value::Number(num) => self.clicks = num,
            crate::value::Value::Text(text) => self.set_value(text),
            _ => {}
        }
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

    fn render_brackets(&self) -> bool {
        false
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.click();
                KeyResult::Handled
            }
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, _theme: &crate::theme::Theme) -> Vec<Span> {
        let label = if self.text.is_empty() {
            " ".to_string()
        } else {
            self.text.clone()
        };

        let mut text = if self.base.focused {
            format!("[ {} ]", label)
        } else {
            label
        };
        let width = text.width();
        if width < self.base.min_width {
            let padding = self.base.min_width - width;
            text.push_str(&" ".repeat(padding));
        }

        let inactive_style = Style::new()
            .with_colors(Color::Rgb(235, 235, 240), Color::Rgb(32, 40, 58))
            .with_dim();
        let active_style = Style::new()
            .with_colors(Color::Rgb(250, 250, 250), Color::Rgb(54, 92, 74))
            .with_bold();

        let style = if self.base.focused {
            active_style
        } else {
            inactive_style
        };

        vec![Span::new(text).with_style(style)]
    }

    fn cursor_offset_in_content(&self) -> usize {
        if self.base.focused { 1 } else { 0 }
    }
}
