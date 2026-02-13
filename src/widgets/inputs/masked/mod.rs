mod format;
mod model;
mod parser;

use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::widgets::base::InputBase;
use crate::widgets::inputs::text_edit;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use crate::widgets::validators::Validator;
use model::{MaskToken, SegmentKind};
use unicode_width::UnicodeWidthStr;

const INVALID_MASK_MESSAGE: &str = "Invalid or incomplete value";

pub struct MaskedInput {
    base: InputBase,
    tokens: Vec<MaskToken>,
    cursor_token: usize,
    cursor_offset: usize,
    submit_target: Option<String>,
    validators: Vec<Validator>,
}

impl MaskedInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, mask: impl Into<String>) -> Self {
        let tokens = parser::parse_mask(mask.into().as_str());
        let cursor_token = format::first_segment_pos(tokens.as_slice()).unwrap_or(0);
        Self {
            base: InputBase::new(id, label),
            tokens,
            cursor_token,
            cursor_offset: 0,
            submit_target: None,
            validators: Vec::new(),
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

    pub fn ipv4(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(
            id,
            label,
            "#{1,3:0-255}.#{1,3:0-255}.#{1,3:0-255}.#{1,3:0-255}",
        )
    }

    pub fn phone_us(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, "(#{3}) #{3}-#{4}")
    }

    pub fn zip_us(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, "#{5}")
    }

    pub fn date_dd_mm_yyyy(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, "DD/MM/YYYY")
    }

    pub fn time_hh_mm(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, "HH:mm")
    }

    pub fn number(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, "#{}")
    }

    fn current_segment(&self) -> Option<&model::SegmentSpec> {
        match self.tokens.get(self.cursor_token) {
            Some(MaskToken::Segment(segment)) => Some(segment),
            _ => None,
        }
    }

    fn current_segment_mut(&mut self) -> Option<&mut model::SegmentSpec> {
        match self.tokens.get_mut(self.cursor_token) {
            Some(MaskToken::Segment(segment)) => Some(segment),
            _ => None,
        }
    }

    fn clear_segments(&mut self) {
        for token in &mut self.tokens {
            if let MaskToken::Segment(segment) = token {
                segment.value.clear();
            }
        }
        self.cursor_offset = 0;
    }

    fn clamp_cursor(&mut self) {
        if let Some(segment) = self.current_segment() {
            self.cursor_offset = self
                .cursor_offset
                .min(text_edit::char_count(segment.value.as_str()));
        } else {
            self.cursor_offset = 0;
        }
    }

    fn insert_char(&mut self, ch: char) -> bool {
        let token_idx = self.cursor_token;
        let cursor_offset = self.cursor_offset;
        let next_segment = format::next_segment_pos(self.tokens.as_slice(), token_idx);

        let Some(segment) = self.current_segment_mut() else {
            return false;
        };
        if !format::token_accepts(segment.kind, ch) {
            return false;
        }

        if let Some(max_len) = segment.max_len
            && text_edit::char_count(segment.value.as_str()) >= max_len
        {
            segment.value.clear();
        }

        let mut next_cursor = cursor_offset;
        text_edit::insert_char(&mut segment.value, &mut next_cursor, ch);
        let new_len = text_edit::char_count(segment.value.as_str());

        if let Some(max_len) = segment.max_len
            && new_len >= max_len
        {
            if let Some(next_segment) = next_segment {
                self.cursor_token = next_segment;
                self.cursor_offset = 0;
            } else {
                self.cursor_offset = new_len.min(max_len);
            }
            return true;
        }

        self.cursor_offset = next_cursor;
        true
    }

    fn delete_prev(&mut self) -> bool {
        let token_idx = self.cursor_token;
        let cursor_offset = self.cursor_offset;

        if cursor_offset > 0
            && let Some(segment) = self.current_segment_mut()
        {
            let mut next_cursor = cursor_offset;
            if text_edit::backspace_char(&mut segment.value, &mut next_cursor) {
                self.cursor_offset = next_cursor;
                return true;
            }
        }

        if let Some(prev_segment) = format::prev_segment_pos(self.tokens.as_slice(), token_idx) {
            self.cursor_token = prev_segment;
            self.cursor_offset = self
                .current_segment()
                .map(|segment| text_edit::char_count(segment.value.as_str()))
                .unwrap_or(0);
            return self.delete_prev();
        }

        false
    }

    fn delete_current(&mut self) -> bool {
        let cursor = self.cursor_offset;
        let Some(segment) = self.current_segment_mut() else {
            return false;
        };
        let mut next_cursor = cursor;
        let changed = delete_char(&mut segment.value, &mut next_cursor);
        self.cursor_offset = next_cursor;
        changed
    }

    fn move_left(&mut self) -> bool {
        if self.cursor_offset > 0 {
            self.cursor_offset -= 1;
            return true;
        }

        if let Some(prev_segment) =
            format::prev_segment_pos(self.tokens.as_slice(), self.cursor_token)
        {
            self.cursor_token = prev_segment;
            self.cursor_offset = self
                .current_segment()
                .map(|segment| text_edit::char_count(segment.value.as_str()))
                .unwrap_or(0);
            return true;
        }

        false
    }

    fn move_right(&mut self) -> bool {
        if let Some(segment) = self.current_segment()
            && self.cursor_offset < text_edit::char_count(segment.value.as_str())
        {
            self.cursor_offset += 1;
            return true;
        }

        if let Some(next_segment) =
            format::next_segment_pos(self.tokens.as_slice(), self.cursor_token)
        {
            self.cursor_token = next_segment;
            self.cursor_offset = 0;
            return true;
        }

        false
    }

    fn increment_current(&mut self, delta: i64) -> bool {
        let Some(segment) = self.current_segment_mut() else {
            return false;
        };

        let SegmentKind::NumericRange { min, max } = segment.kind else {
            return false;
        };

        let current = segment.value.parse::<i64>().unwrap_or(min);
        let mut next = current + delta;
        if next > max {
            next = min;
        } else if next < min {
            next = max;
        }

        if segment.min_len > 0 {
            segment.value = format!("{:0width$}", next, width = segment.min_len);
        } else {
            segment.value = next.to_string();
        }
        self.cursor_offset = text_edit::char_count(segment.value.as_str());
        true
    }

    fn set_from_text(&mut self, value: &str) -> bool {
        let chars: Vec<char> = value.chars().collect();
        let mut idx = 0usize;
        let next_literals = (0..self.tokens.len())
            .map(|token_idx| format::next_literal_char(self.tokens.as_slice(), token_idx))
            .collect::<Vec<_>>();

        let mut parsed = vec![None::<String>; self.tokens.len()];
        for (token_idx, token) in self.tokens.iter().enumerate() {
            match token {
                MaskToken::Literal(ch) => {
                    if chars.get(idx).copied() != Some(*ch) {
                        return false;
                    }
                    idx += 1;
                }
                MaskToken::Segment(segment) => {
                    let mut out = String::new();
                    let next_literal = next_literals[token_idx];
                    while idx < chars.len() {
                        let ch = chars[idx];
                        if let Some(next_literal) = next_literal
                            && ch == next_literal
                        {
                            break;
                        }
                        if let Some(max_len) = segment.max_len
                            && text_edit::char_count(out.as_str()) >= max_len
                        {
                            break;
                        }
                        if format::token_accepts(segment.kind, ch) {
                            out.push(ch);
                            idx += 1;
                        } else {
                            break;
                        }
                    }
                    parsed[token_idx] = Some(out);
                }
            }
        }

        for (token_idx, token) in self.tokens.iter_mut().enumerate() {
            if let MaskToken::Segment(segment) = token {
                segment.value = parsed[token_idx].take().unwrap_or_default();
            }
        }

        self.clamp_cursor();
        true
    }

    fn validated_value(&self) -> Result<String, String> {
        if !format::has_any_segment_input(self.tokens.as_slice()) {
            return Ok(String::new());
        }

        let Some(value) = format::formatted_complete_value(self.tokens.as_slice()) else {
            return Err(INVALID_MASK_MESSAGE.to_string());
        };

        Ok(value)
    }
}

impl Drawable for MaskedInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line_state = self.base.line_state(ctx);
        let mut line = vec![Span::new(line_state.prefix).no_wrap()];
        line.extend(format::render_spans(self.tokens.as_slice()));

        DrawOutput { lines: vec![line] }
    }
}

impl Interactive for MaskedInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(ch) => {
                if self.insert_char(ch) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Backspace => {
                if self.delete_prev() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Delete => {
                if self.delete_current() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Left => {
                if self.move_left() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if self.move_right() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Up => {
                if self.increment_current(1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Down => {
                if self.increment_current(-1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter => {
                let value =
                    format::formatted_complete_value(self.tokens.as_slice()).unwrap_or_default();
                InteractionResult::submit_or_produce(
                    self.submit_target.as_deref(),
                    Value::Text(value),
                )
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        let value = format::formatted_complete_value(self.tokens.as_slice()).unwrap_or_default();
        Some(Value::Text(value))
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::Text(text) => {
                let _ = self.set_from_text(text.as_str());
            }
            Value::None => self.clear_segments(),
            _ => {}
        }
        self.clamp_cursor();
    }

    fn validate_live(&self) -> Result<(), String> {
        let value = self.validated_value()?;
        for validator in &self.validators {
            validator(value.as_str())?;
        }
        Ok(())
    }

    fn validate_submit(&self) -> Result<(), String> {
        let value = self.validated_value()?;
        for validator in &self.validators {
            validator(value.as_str())?;
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let prefix = format!("{} {}: ", self.base.focus_marker(true), self.base.label());
        let local_offset = format::cursor_offset(
            self.tokens.as_slice(),
            self.cursor_token,
            self.cursor_offset,
        );
        Some(CursorPos {
            col: (UnicodeWidthStr::width(prefix.as_str()) + local_offset) as u16,
            row: 0,
        })
    }
}

fn delete_char(value: &mut String, cursor: &mut usize) -> bool {
    let pos = text_edit::clamp_cursor(*cursor, value.as_str());
    let len = text_edit::char_count(value.as_str());
    if pos >= len {
        *cursor = pos;
        return false;
    }

    let byte_pos = byte_index_at_char(value.as_str(), pos);
    value.remove(byte_pos);
    *cursor = pos;
    true
}

fn byte_index_at_char(value: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }

    value
        .char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(value.len())
}
