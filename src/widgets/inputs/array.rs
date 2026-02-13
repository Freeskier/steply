use super::text_edit;
use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, TextAction,
};
use crate::widgets::validators::Validator;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct ArrayInput {
    base: InputBase,
    items: Vec<String>,
    active: usize,
    cursor: usize,
    validators: Vec<Validator>,
}

impl ArrayInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            items: vec![String::new()],
            active: 0,
            cursor: 0,
            validators: Vec::new(),
        }
    }

    pub fn with_items(mut self, items: Vec<String>) -> Self {
        self.replace_items(items);
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn replace_items(&mut self, items: Vec<String>) {
        let cleaned = items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();

        if cleaned.is_empty() {
            self.items = vec![String::new()];
            self.active = 0;
            self.cursor = 0;
            return;
        }

        self.items = cleaned;
        self.active = 0;
        self.cursor = text_edit::char_count(self.items[0].as_str());
    }

    fn ensure_invariants(&mut self) {
        if self.items.is_empty() {
            self.items.push(String::new());
        }
        if self.active >= self.items.len() {
            self.active = self.items.len().saturating_sub(1);
        }
        if let Some(item) = self.items.get(self.active) {
            self.cursor = text_edit::clamp_cursor(self.cursor, item.as_str());
        } else {
            self.cursor = 0;
        }
    }

    fn active_item(&self) -> &str {
        self.items
            .get(self.active)
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn active_item_mut(&mut self) -> &mut String {
        self.ensure_invariants();
        &mut self.items[self.active]
    }

    fn split_active(&mut self) {
        self.ensure_invariants();
        let current = self.active_item().to_string();
        let split_at = self.cursor.min(text_edit::char_count(current.as_str()));
        let left = current.chars().take(split_at).collect::<String>();
        let right = current.chars().skip(split_at).collect::<String>();

        self.items[self.active] = left.trim().to_string();
        let insert_at = self.active + 1;
        self.items.insert(insert_at, right.trim().to_string());
        self.active = insert_at;
        self.cursor = text_edit::char_count(self.items[self.active].as_str());
        self.normalize_items();
    }

    fn normalize_items(&mut self) {
        for item in &mut self.items {
            *item = item.trim().to_string();
        }
        self.ensure_invariants();
    }

    fn insert_char(&mut self, ch: char) {
        let mut cursor = self.cursor;
        text_edit::insert_char(self.active_item_mut(), &mut cursor, ch);
        self.cursor = cursor;
    }

    fn backspace(&mut self) -> bool {
        self.ensure_invariants();
        let mut cursor = self.cursor;
        if text_edit::backspace_char(self.active_item_mut(), &mut cursor) {
            self.cursor = cursor;
            return true;
        }

        if self.active == 0 {
            return false;
        }

        let current = self.items.remove(self.active);
        self.active -= 1;
        let previous_len = text_edit::char_count(self.items[self.active].as_str());
        self.cursor = previous_len;

        if !current.trim().is_empty() {
            self.items[self.active].push_str(current.as_str());
            self.cursor = text_edit::char_count(self.items[self.active].as_str());
        }
        true
    }

    fn delete_forward(&mut self) -> bool {
        self.ensure_invariants();

        let mut cursor = self.cursor;
        if text_edit::delete_char(self.active_item_mut(), &mut cursor) {
            self.cursor = cursor;
            return true;
        }

        if self.active + 1 >= self.items.len() {
            return false;
        }

        let next = self.items.remove(self.active + 1);
        if !next.trim().is_empty() {
            self.items[self.active].push_str(next.as_str());
        }
        true
    }

    fn move_left(&mut self) {
        self.ensure_invariants();
        if self.cursor > 0 {
            self.cursor -= 1;
            return;
        }
        if self.active > 0 {
            self.active -= 1;
            self.cursor = text_edit::char_count(self.items[self.active].as_str());
        }
    }

    fn move_right(&mut self) {
        self.ensure_invariants();
        let active_len = text_edit::char_count(self.active_item());
        if self.cursor < active_len {
            self.cursor += 1;
            return;
        }
        if self.active + 1 < self.items.len() {
            self.active += 1;
            self.cursor = 0;
        }
    }

    fn remove_active(&mut self) -> bool {
        self.ensure_invariants();
        if self.items.len() == 1 {
            self.items[0].clear();
            self.active = 0;
            self.cursor = 0;
            return true;
        }

        self.items.remove(self.active);
        if self.active >= self.items.len() {
            self.active = self.items.len() - 1;
        }
        self.cursor = text_edit::char_count(self.items[self.active].as_str());
        true
    }

    fn remove_next(&mut self) -> bool {
        self.ensure_invariants();
        if self.active + 1 >= self.items.len() {
            return false;
        }
        self.items.remove(self.active + 1);
        true
    }

    fn build_content(&self, focused: bool) -> (Vec<Span>, usize) {
        let mut spans = Vec::<Span>::new();
        let mut width = 0usize;
        let mut cursor_width = 0usize;

        spans.push(Span::new("(").no_wrap());
        width += 1;

        for (idx, item) in self.items.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::new(", ").no_wrap());
                width += 2;
            }

            let display = if item.is_empty() {
                " ".to_string()
            } else {
                item.clone()
            };
            let is_active = focused && idx == self.active;
            let style = if is_active {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::default()
            };

            if is_active {
                let cursor_chars = self.cursor.min(text_edit::char_count(item.as_str()));
                cursor_width = width + width_of_char_prefix(item.as_str(), cursor_chars);
            }

            width += UnicodeWidthStr::width(display.as_str());
            spans.push(Span::styled(display, style).no_wrap());
        }

        spans.push(Span::new(")").no_wrap());

        (spans, cursor_width)
    }
}

impl Drawable for ArrayInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line = self.base.line_state(ctx);

        let (content, _) = self.build_content(line.focused);
        let mut spans = vec![Span::new(line.prefix).no_wrap()];
        spans.extend(content);
        DrawOutput { lines: vec![spans] }
    }
}

impl Interactive for ArrayInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(',') | KeyCode::Char(';') => {
                self.split_active();
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if self.backspace() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Delete => {
                if self.delete_forward() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Left => {
                self.move_left();
                InteractionResult::handled()
            }
            KeyCode::Right => {
                self.move_right();
                InteractionResult::handled()
            }
            KeyCode::Char(ch) => {
                if ch.is_control() {
                    return InteractionResult::ignored();
                }
                self.insert_char(ch);
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::submit_requested(),
            _ => InteractionResult::ignored(),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let changed = match action {
            TextAction::DeleteWordLeft => self.remove_active(),
            TextAction::DeleteWordRight => self.remove_next(),
        };
        if changed {
            self.normalize_items();
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::List(
            self.items
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .map(Value::Text)
                .collect::<Vec<_>>(),
        ))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(items) = value.to_text_list() {
            self.replace_items(items);
            return;
        }
        if let Some(text) = value.as_text() {
            let parts = text
                .split(&[',', ';'][..])
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            self.replace_items(parts);
            return;
        }
        if matches!(value, Value::None) {
            self.replace_items(Vec::new());
        }
    }

    fn validate_submit(&self) -> Result<(), String> {
        for item in &self.items {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }
            for validator in &self.validators {
                validator(trimmed)?;
            }
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let prefix = format!("{} {}: ", self.base.focus_marker(true), self.base.label());
        let (_, cursor_offset) = self.build_content(true);
        Some(CursorPos {
            col: (UnicodeWidthStr::width(prefix.as_str()) + cursor_offset) as u16,
            row: 0,
        })
    }
}

fn width_of_char_prefix(value: &str, chars: usize) -> usize {
    value
        .chars()
        .take(chars)
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}
