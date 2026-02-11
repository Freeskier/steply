use super::text_edit;
use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

pub struct ArrayInput {
    base: InputBase,
    items: Vec<String>,
    active: usize,
    cursor: usize,
}

impl ArrayInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            items: vec![String::new()],
            active: 0,
            cursor: 0,
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

    pub fn with_items(mut self, items: Vec<String>) -> Self {
        let cleaned: Vec<String> = items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
        self.replace_items(cleaned);
        self
    }

    fn replace_items(&mut self, items: Vec<String>) {
        if items.is_empty() {
            self.items = vec![String::new()];
            self.active = 0;
            self.cursor = 0;
            return;
        }

        self.items = items;
        self.active = 0;
        self.cursor = text_edit::char_count(&self.items[0]);
    }

    fn active_item(&self) -> &str {
        self.items
            .get(self.active)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn active_item_mut(&mut self) -> &mut String {
        if self.items.is_empty() {
            self.items.push(String::new());
            self.active = 0;
        }
        if self.active >= self.items.len() {
            self.active = self.items.len().saturating_sub(1);
        }
        &mut self.items[self.active]
    }

    fn normalize_items(&mut self) {
        for item in &mut self.items {
            *item = item.trim().to_string();
        }

        if self.items.is_empty() {
            self.items.push(String::new());
            self.active = 0;
            self.cursor = 0;
        } else if self.active >= self.items.len() {
            self.active = self.items.len() - 1;
            self.cursor = text_edit::char_count(&self.items[self.active]);
        }
    }

    fn split_active(&mut self) {
        let current = self.active_item().to_string();
        let (mut left, mut right) = text_edit::split_at_char(&current, self.cursor);
        left = left.trim().to_string();
        right = right.trim().to_string();

        if left.is_empty() && right.is_empty() {
            self.active_item_mut().clear();
        } else {
            self.active_item_mut().clear();
            self.active_item_mut().push_str(&left);
        }

        let insert_at = self.active + 1;
        self.items.insert(insert_at, right);
        self.active = insert_at;
        self.cursor = text_edit::char_count(&self.items[self.active]);
        self.normalize_items();
    }

    fn remove_active(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }

        self.items.remove(self.active);
        if self.items.is_empty() {
            self.items.push(String::new());
            self.active = 0;
            self.cursor = 0;
            return true;
        }

        if self.active >= self.items.len() {
            self.active = self.items.len() - 1;
        }
        self.cursor = text_edit::char_count(&self.items[self.active]);
        true
    }

    fn remove_next(&mut self) -> bool {
        if self.items.len() <= 1 {
            return false;
        }
        if self.active + 1 < self.items.len() {
            self.items.remove(self.active + 1);
            return true;
        }
        false
    }

    fn insert_char(&mut self, ch: char) {
        let idx = self.active;
        if let Some(item) = self.items.get_mut(idx) {
            text_edit::insert_char(item, &mut self.cursor, ch);
        }
    }

    fn backspace(&mut self) -> bool {
        let idx = self.active;
        if let Some(item) = self.items.get_mut(idx) {
            if text_edit::backspace_char(item, &mut self.cursor) {
                return true;
            }
        }

        if idx > 0 {
            let prev = idx - 1;
            let current = self.items.remove(idx);
            self.active = prev;
            let prev_len = text_edit::char_count(&self.items[prev]);
            self.cursor = prev_len;
            if !current.trim().is_empty() {
                if let Some(item) = self.items.get_mut(prev) {
                    item.push_str(&current);
                    self.cursor = text_edit::char_count(item);
                }
            }
            return true;
        }

        false
    }

    fn delete_forward(&mut self) -> bool {
        let idx = self.active;
        if let Some(item) = self.items.get_mut(idx) {
            if text_edit::delete_char(item, &mut self.cursor) {
                return true;
            }
        }

        if idx + 1 < self.items.len() {
            let next = self.items.remove(idx + 1);
            if !next.trim().is_empty() {
                if let Some(item) = self.items.get_mut(idx) {
                    item.push_str(&next);
                }
            }
            return true;
        }

        false
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            return;
        }
        if self.active > 0 {
            self.active -= 1;
            self.cursor = text_edit::char_count(&self.items[self.active]);
        }
    }

    fn move_right(&mut self) {
        let len = text_edit::char_count(self.active_item());
        if self.cursor < len {
            self.cursor += 1;
            return;
        }
        if self.active + 1 < self.items.len() {
            self.active += 1;
            self.cursor = 0;
        }
    }

    fn build_spans(&self, theme: &crate::theme::Theme) -> (Vec<Span>, usize) {
        let mut spans = Vec::new();
        let mut offset = 0usize;
        let mut cursor_offset = 0usize;

        let show_active_brackets = self.base.focused;
        spans.push(Span::new("("));
        offset += 1;
        for (idx, item) in self.items.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::new(", "));
                offset += 2;
            }

            let is_active = idx == self.active;
            if is_active && show_active_brackets {
                spans.push(Span::new("["));
                offset += 1;
            }

            let mut content = item.clone();
            if content.is_empty() {
                content = " ".to_string();
            }

            let mut span = Span::new(content.clone());
            if is_active && self.base.focused {
                span = span.with_style(theme.focused.clone());
            }
            spans.push(span);

            if is_active && self.base.focused {
                cursor_offset = offset + self.cursor.min(text_edit::char_count(&content));
            }

            offset += content.width();
            if is_active && show_active_brackets {
                spans.push(Span::new("]"));
                offset += 1;
            }
        }
        spans.push(Span::new(")"));

        (spans, cursor_offset)
    }
}

impl Input for ArrayInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        self.items
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn set_value(&mut self, value: String) {
        let parts: Vec<String> = value
            .split(&[',', ';'][..])
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
        self.replace_items(parts);
    }

    fn value_typed(&self) -> crate::value::Value {
        crate::value::Value::List(
            self.items
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
        )
    }

    fn set_value_typed(&mut self, value: crate::value::Value) {
        match value {
            crate::value::Value::List(items) => {
                let cleaned: Vec<String> = items
                    .into_iter()
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect();
                self.replace_items(cleaned);
            }
            crate::value::Value::Text(text) => {
                self.set_value(text);
            }
            crate::value::Value::None => {
                self.replace_items(Vec::new());
            }
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
        self.active
    }

    fn render_brackets(&self) -> bool {
        false
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(',') | KeyCode::Char(';') => {
                self.split_active();
                KeyResult::Handled
            }
            KeyCode::Backspace => {
                if self.backspace() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Delete => {
                if self.delete_forward() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Left => {
                self.move_left();
                KeyResult::Handled
            }
            KeyCode::Right => {
                self.move_right();
                KeyResult::Handled
            }
            KeyCode::Char(ch) => {
                if ch.is_control() {
                    KeyResult::NotHandled
                } else {
                    self.insert_char(ch);
                    KeyResult::Handled
                }
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, theme: &crate::theme::Theme) -> Vec<Span> {
        let (spans, _) = self.build_spans(theme);
        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let (_, offset) = self.build_spans(&crate::theme::Theme::default());
        offset
    }

    fn delete_word(&mut self) {
        if self.remove_active() {
            self.normalize_items();
        }
    }

    fn delete_word_forward(&mut self) {
        if self.remove_next() {
            self.normalize_items();
        }
    }

    fn validate_internal(&self) -> Result<(), String> {
        for item in &self.items {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }
            for validator in self.validators() {
                validator(trimmed)?;
            }
        }
        Ok(())
    }
}
