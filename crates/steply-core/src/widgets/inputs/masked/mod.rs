mod format;
mod model;
mod parser;

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::SystemEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::inline::{Inline, InlineGroup};
use crate::ui::span::Span;
use crate::widgets::base::WidgetBase;
use crate::widgets::shared::text_edit;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use model::{MaskToken, SegmentKind};

const INVALID_MASK_MESSAGE: &str = "Invalid or incomplete value";

pub struct MaskedInput {
    base: WidgetBase,
    tokens: Vec<MaskToken>,
    cursor_token: usize,
    cursor_offset: usize,
    submit_target: Option<ValueTarget>,
    validators: Vec<Validator>,
}

impl MaskedInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, mask: impl Into<String>) -> Self {
        let tokens = parser::parse_mask(mask.into().as_str());
        let cursor_token = format::first_segment_pos(tokens.as_slice()).unwrap_or(0);
        Self {
            base: WidgetBase::new(id, label),
            tokens,
            cursor_token,
            cursor_offset: 0,
            submit_target: None,
            validators: Vec::new(),
        }
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
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

    fn current_cursor_limit(&self) -> usize {
        format::cursor_limit(self.tokens.as_slice(), self.cursor_token)
    }

    fn current_max_cursor_pos(&self) -> usize {
        self.current_cursor_limit().saturating_sub(1)
    }

    fn clamp_cursor(&mut self) {
        if self.current_segment().is_some() {
            self.cursor_offset = self.cursor_offset.min(self.current_max_cursor_pos());
        } else {
            self.cursor_offset = 0;
        }
    }

    pub fn focus_first_unfilled(&mut self) -> bool {
        let mut first_segment: Option<(usize, usize)> = None;
        let mut first_unfilled: Option<(usize, usize)> = None;
        for (idx, token) in self.tokens.iter().enumerate() {
            let MaskToken::Segment(segment) = token else {
                continue;
            };
            let used = text_edit::char_count(segment.value.as_str());
            if first_segment.is_none() {
                first_segment = Some((idx, used.saturating_sub(1)));
            }
            let display_len = format::cursor_limit(self.tokens.as_slice(), idx);
            if used < display_len {
                first_unfilled = Some((idx, used));
                break;
            }
        }

        let (token, offset) = first_unfilled.or(first_segment).unwrap_or((0, 0));
        let changed = self.cursor_token != token || self.cursor_offset != offset;
        self.cursor_token = token;
        self.cursor_offset = offset;
        self.clamp_cursor();
        changed
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
                self.cursor_offset = max_len.saturating_sub(1);
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
        let changed = text_edit::delete_char(&mut segment.value, &mut next_cursor);
        self.cursor_offset = next_cursor;
        changed
    }

    fn move_left(&mut self) -> bool {
        if self.cursor_offset > 0 {
            self.cursor_offset -= 1;
            return true;
        }

        self.move_prev_segment_end()
    }

    fn move_right(&mut self) -> bool {
        if self.cursor_offset < self.current_max_cursor_pos() {
            self.cursor_offset += 1;
            return true;
        }

        self.move_next_segment_start()
    }

    fn move_next_segment_start(&mut self) -> bool {
        let Some(next_segment) =
            format::next_segment_pos(self.tokens.as_slice(), self.cursor_token)
        else {
            return false;
        };
        self.cursor_token = next_segment;
        self.cursor_offset = 0;
        true
    }

    fn move_prev_segment_end(&mut self) -> bool {
        let Some(prev_segment) =
            format::prev_segment_pos(self.tokens.as_slice(), self.cursor_token)
        else {
            return false;
        };
        self.cursor_token = prev_segment;
        self.cursor_offset = self.current_max_cursor_pos();
        true
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
        self.cursor_offset = text_edit::char_count(segment.value.as_str()).saturating_sub(1);
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

        if idx != chars.len() {
            return false;
        }

        for (token_idx, token) in self.tokens.iter_mut().enumerate() {
            if let MaskToken::Segment(segment) = token {
                segment.value = parsed[token_idx].take().unwrap_or_default();
            }
        }

        self.clamp_cursor();
        true
    }

    pub fn render_spans(&self) -> Vec<Span> {
        format::render_spans(self.tokens.as_slice(), None)
    }

    pub fn render_spans_with_active(&self, active: bool) -> Vec<Span> {
        let active_segment = if active {
            Some(self.cursor_token)
        } else {
            None
        };
        format::render_spans(self.tokens.as_slice(), active_segment)
    }

    pub fn cursor_col(&self) -> usize {
        format::cursor_offset(
            self.tokens.as_slice(),
            self.cursor_token,
            self.cursor_offset,
        )
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

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        if self.base.is_focused(ctx) {
            let spans = self.render_spans_with_active(true);
            let group = Inline::group(InlineGroup::no_break(
                spans.into_iter().map(Inline::from).collect(),
            ));
            DrawOutput::with_inline_lines(vec![vec![group]])
        } else {
            let plain = format::render_plain_value(self.tokens.as_slice());
            DrawOutput::with_lines(vec![vec![Span::new(plain).no_wrap()]])
        }
    }
}

impl Interactive for MaskedInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn submit_target(&self) -> Option<&ValueTarget> {
        self.submit_target.as_ref()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Tab => {
                InteractionResult::handled_if(if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.move_prev_segment_end()
                } else {
                    self.move_next_segment_start()
                })
            }
            KeyCode::BackTab => InteractionResult::handled_if(self.move_prev_segment_end()),
            KeyCode::Char(ch) => InteractionResult::handled_if(self.insert_char(ch)),
            KeyCode::Backspace => InteractionResult::handled_if(self.delete_prev()),
            KeyCode::Delete => InteractionResult::handled_if(self.delete_current()),
            KeyCode::Left => InteractionResult::handled_if(self.move_left()),
            KeyCode::Right => InteractionResult::handled_if(self.move_right()),
            KeyCode::Up => InteractionResult::handled_if(self.increment_current(1)),
            KeyCode::Down => InteractionResult::handled_if(self.increment_current(-1)),
            KeyCode::Enter => {
                let value =
                    format::formatted_complete_value(self.tokens.as_slice()).unwrap_or_default();
                InteractionResult::submit_or_produce(
                    self.submit_target.as_ref(),
                    Value::Text(value),
                )
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        if let SystemEvent::RequestFocus { target } = event
            && target
                .as_ref()
                .is_some_and(|target| target.as_str() == self.base.id())
        {
            return InteractionResult::handled_if(self.focus_first_unfilled());
        }
        InteractionResult::ignored()
    }

    fn value(&self) -> Option<Value> {
        let value = format::formatted_complete_value(self.tokens.as_slice()).unwrap_or_default();
        Some(Value::Text(value))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.as_text() {
            let _ = self.set_from_text(text);
        } else if matches!(value, Value::None) {
            self.clear_segments();
        }
        self.clamp_cursor();
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        let value = self.validated_value()?;
        run_validators(&self.validators, &Value::Text(value))
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let local_offset = format::cursor_offset(
            self.tokens.as_slice(),
            self.cursor_token,
            self.cursor_offset,
        );
        Some(CursorPos {
            col: local_offset as u16,
            row: 0,
        })
    }
}
