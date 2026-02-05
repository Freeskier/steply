use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::style::{Color, Style};
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

pub struct ChoiceInput {
    base: InputBase,
    options: Vec<String>,
    selected: usize,
    show_bullets: bool,
}

impl ChoiceInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            options,
            selected: 0,
            show_bullets: true,
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

    pub fn with_bullets(mut self, enabled: bool) -> Self {
        self.show_bullets = enabled;
        self
    }

    fn move_left(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let len = self.options.len();
        self.selected = (self.selected + len - 1) % len;
        self.base.error = None;
    }

    fn move_right(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let len = self.options.len();
        self.selected = (self.selected + 1) % len;
        self.base.error = None;
    }

    fn select_by_letter(&mut self, ch: char) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let needle = ch.to_ascii_lowercase();
        if let Some((idx, _)) = self.options.iter().enumerate().find(|(_, opt)| {
            opt.chars()
                .next()
                .is_some_and(|c| c.to_ascii_lowercase() == needle)
        }) {
            self.selected = idx;
            self.base.error = None;
            return true;
        }
        false
    }

    fn current_option(&self) -> Option<&str> {
        self.options.get(self.selected).map(|s| s.as_str())
    }
}

impl Input for ChoiceInput {
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
        self.selected
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Left | KeyCode::Up => {
                self.move_left();
                KeyResult::Handled
            }
            KeyCode::Right | KeyCode::Down => {
                self.move_right();
                KeyResult::Handled
            }
            KeyCode::Char(ch) => {
                if self.select_by_letter(ch) {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, theme: &crate::theme::Theme) -> Vec<Span> {
        let mut spans = Vec::new();
        let active_style = theme.focused.clone();
        let inactive_style = theme.placeholder.clone();
        let active_bullet = Span::new("●").with_style(Style::new().with_color(Color::Green));
        let inactive_bullet = Span::new("○").with_style(inactive_style.clone());

        for (idx, option) in self.options.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::new(" / "));
            }
            if self.show_bullets {
                if idx == self.selected {
                    spans.push(active_bullet.clone());
                } else {
                    spans.push(inactive_bullet.clone());
                }
                spans.push(Span::new(" "));
            }

            let mut text_span = Span::new(option.clone());
            if idx == self.selected {
                text_span = text_span.with_style(active_style.clone());
            } else {
                text_span = text_span.with_style(inactive_style.clone());
            }
            spans.push(text_span);
        }

        let width: usize = spans.iter().map(|s| s.text().width()).sum();
        if width < self.base.min_width {
            spans.push(Span::new(" ".repeat(self.base.min_width - width)));
        }

        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let mut offset = 0usize;
        for (idx, option) in self.options.iter().enumerate() {
            if idx > 0 {
                offset += 3;
            }
            if self.show_bullets {
                offset += 2;
            }
            if idx == self.selected {
                return offset;
            }
            offset += option.width();
        }
        offset
    }
}
