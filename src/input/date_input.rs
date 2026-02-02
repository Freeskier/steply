use crate::input::{Input, InputBase, KeyResult, NodeId};
use crate::span::Span;
use crate::style::Style;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::theme;
use crate::validators::Validator;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

impl SegmentType {
    fn min_value(&self) -> u32 {
        match self {
            SegmentType::Year => 1900,
            SegmentType::Month | SegmentType::Day => 1,
            _ => 0,
        }
    }

    fn max_value(&self) -> u32 {
        match self {
            SegmentType::Year => 2100,
            SegmentType::Month => 12,
            SegmentType::Day => 31,
            SegmentType::Hour => 23,
            SegmentType::Minute | SegmentType::Second => 59,
        }
    }

    fn length(&self) -> usize {
        match self {
            SegmentType::Year => 4,
            _ => 2,
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "YYYY" => Some(SegmentType::Year),
            "MM" => Some(SegmentType::Month),
            "DD" => Some(SegmentType::Day),
            "HH" => Some(SegmentType::Hour),
            "mm" => Some(SegmentType::Minute),
            "ss" => Some(SegmentType::Second),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct DateSegment {
    segment_type: SegmentType,
    value: String,
}

impl DateSegment {
    fn new(segment_type: SegmentType) -> Self {
        Self {
            segment_type,
            value: String::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    fn is_complete(&self) -> bool {
        self.value.len() == self.segment_type.length()
    }

    fn placeholder(&self) -> &'static str {
        match self.segment_type {
            SegmentType::Year => "yyyy",
            SegmentType::Month => "mm",
            SegmentType::Day => "dd",
            SegmentType::Hour => "hh",
            SegmentType::Minute => "mm",
            SegmentType::Second => "ss",
        }
    }

    fn numeric_value(&self) -> u32 {
        self.value.parse().unwrap_or(0)
    }

    fn increment(&mut self) {
        let current = self.numeric_value();
        let max = self.segment_type.max_value();
        let min = self.segment_type.min_value();
        let next = if current >= max || current < min {
            min
        } else {
            current + 1
        };
        self.value = format!("{:0width$}", next, width = self.segment_type.length());
    }

    fn decrement(&mut self) {
        let current = self.numeric_value();
        let max = self.segment_type.max_value();
        let min = self.segment_type.min_value();
        let prev = if current <= min || current == 0 {
            max
        } else {
            current - 1
        };
        self.value = format!("{:0width$}", prev, width = self.segment_type.length());
    }

    fn insert_digit(&mut self, digit: char) -> bool {
        if !digit.is_ascii_digit() {
            return false;
        }
        let max_len = self.segment_type.length();
        if self.value.len() >= max_len {
            self.value = digit.to_string();
            return true;
        }
        self.value.push(digit);
        if let Ok(val) = self.value.parse::<u32>() {
            if val > self.segment_type.max_value() {
                self.value = digit.to_string();
            }
        }
        true
    }

    fn delete_digit(&mut self) -> bool {
        if self.value.is_empty() {
            return false;
        }
        self.value.pop();
        true
    }

    fn display_string(&self) -> String {
        let len = self.segment_type.length();
        if self.value.is_empty() {
            self.placeholder().to_string()
        } else if self.value.len() < len {
            let placeholder = self.placeholder();
            format!("{}{}", self.value, &placeholder[self.value.len()..len])
        } else {
            self.value.clone()
        }
    }

    fn normalize(&mut self) {
        if self.value.is_empty() {
            return;
        }
        let len = self.segment_type.length();
        if self.value.len() < len {
            if let Ok(val) = self.value.parse::<u32>() {
                self.value = format!("{:0width$}", val, width = len);
            }
        }
    }
}

pub struct DateTimeInput {
    base: InputBase,
    format: String,
    segments: Vec<DateSegment>,
    separators: Vec<String>,
    focused_segment: usize,
}

pub type DateInput = DateTimeInput;

impl DateTimeInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, format: impl Into<String>) -> Self {
        let format_str = format.into();
        let (segments, separators) = Self::parse_format(&format_str);

        Self {
            base: InputBase::new(id, label),
            format: format_str,
            segments,
            separators,
            focused_segment: 0,
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

    fn parse_format(format: &str) -> (Vec<DateSegment>, Vec<String>) {
        let mut segments = Vec::new();
        let mut separators = Vec::new();
        let mut current_sep = String::new();
        let mut chars = format.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_alphabetic() {
                let mut token = String::from(ch);
                while let Some(&next_ch) = chars.peek() {
                    if next_ch == ch {
                        token.push(next_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(seg_type) = SegmentType::from_token(&token) {
                    separators.push(current_sep.clone());
                    current_sep.clear();
                    segments.push(DateSegment::new(seg_type));
                } else {
                    current_sep.push_str(&token);
                }
            } else {
                current_sep.push(ch);
            }
        }
        separators.push(current_sep);
        (segments, separators)
    }

    pub fn display_string(&self) -> String {
        let mut result = String::new();
        for (i, segment) in self.segments.iter().enumerate() {
            if i < self.separators.len() {
                result.push_str(&self.separators[i]);
            }
            result.push_str(&segment.display_string());
        }
        if self.segments.len() < self.separators.len() {
            result.push_str(&self.separators[self.segments.len()]);
        }
        result
    }

    fn format_value(&self) -> String {
        let mut result = String::new();
        for (i, segment) in self.segments.iter().enumerate() {
            if i < self.separators.len() {
                result.push_str(&self.separators[i]);
            }
            result.push_str(&segment.value);
        }
        if self.segments.len() < self.separators.len() {
            result.push_str(&self.separators[self.segments.len()]);
        }
        result
    }

    fn is_complete_internal(&self) -> bool {
        self.segments.iter().all(|s| s.is_complete())
    }

    fn move_next(&mut self) -> bool {
        if let Some(segment) = self.segments.get_mut(self.focused_segment) {
            segment.normalize();
        }

        if self.focused_segment + 1 < self.segments.len() {
            self.focused_segment += 1;
            true
        } else {
            false
        }
    }

    fn move_prev(&mut self) -> bool {
        if self.focused_segment > 0 {
            self.focused_segment -= 1;
            true
        } else {
            false
        }
    }
}

impl Input for DateTimeInput {
    fn id(&self) -> &NodeId {
        &self.base.id
    }

    fn label(&self) -> &str {
        &self.base.label
    }

    fn value(&self) -> String {
        if self.is_complete_internal() {
            self.format_value()
        } else {
            String::new()
        }
    }

    fn set_value(&mut self, value: String) {
        if !self.format.is_empty() && value.len() != self.format.len() {
            return;
        }

        for segment in &mut self.segments {
            segment.value.clear();
        }

        let mut pos = 0usize;
        for (i, segment) in self.segments.iter_mut().enumerate() {
            if pos > value.len() {
                return;
            }
            let sep = self.separators.get(i).map(String::as_str).unwrap_or("");
            if !sep.is_empty() {
                if value[pos..].starts_with(sep) {
                    pos += sep.len();
                } else {
                    return;
                }
            }

            let len = segment.segment_type.length();
            if pos + len > value.len() {
                return;
            }
            let part = &value[pos..pos + len];
            if part.chars().all(|c| c.is_ascii_digit()) {
                segment.value = part.to_string();
            } else {
                return;
            }
            pos += len;
        }

        if let Some(trailing) = self.separators.get(self.segments.len()) {
            if !trailing.is_empty() && value[pos..].starts_with(trailing) {}
        }
    }

    fn raw_value(&self) -> String {
        if self.segments.iter().all(|seg| seg.is_empty()) {
            String::new()
        } else {
            self.format_value()
        }
    }

    fn is_complete(&self) -> bool {
        self.is_complete_internal()
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
        self.focused_segment
    }

    fn min_width(&self) -> usize {
        self.base.min_width
    }

    fn validators(&self) -> &[Validator] {
        &self.base.validators
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(ch) if ch.is_ascii_digit() => {
                if let Some(segment) = self.segments.get_mut(self.focused_segment) {
                    segment.insert_digit(ch);
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Backspace => {
                if let Some(segment) = self.segments.get_mut(self.focused_segment) {
                    segment.delete_digit();
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Left => {
                if self.move_prev() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Right | KeyCode::Char('/') | KeyCode::Char(':') => {
                if self.move_next() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Up => {
                if let Some(segment) = self.segments.get_mut(self.focused_segment) {
                    segment.increment();
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Down => {
                if let Some(segment) = self.segments.get_mut(self.focused_segment) {
                    segment.decrement();
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Enter => {
                if let Some(segment) = self.segments.get_mut(self.focused_segment) {
                    segment.normalize();
                }
                KeyResult::Submit
            }
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self) -> Vec<Span> {
        let mut spans = Vec::new();
        let theme = theme::Theme::default_theme();

        for (i, segment) in self.segments.iter().enumerate() {
            if i < self.separators.len() && !self.separators[i].is_empty() {
                spans.push(Span::new(&self.separators[i]));
            }

            let mut style = if segment.is_empty() {
                theme.placeholder.clone()
            } else {
                Style::default()
            };

            if i == self.focused_segment && self.base.focused {
                style = style.merge(&theme.focused);
            }

            spans.push(Span::new(segment.display_string()).with_style(style));
        }

        if self.segments.len() < self.separators.len() {
            let last_sep = &self.separators[self.segments.len()];
            if !last_sep.is_empty() {
                spans.push(Span::new(last_sep));
            }
        }

        let content_width = self.display_string().width();
        if content_width < self.base.min_width {
            let padding = self.base.min_width - content_width;
            spans.push(Span::new(" ".repeat(padding)));
        }

        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let mut offset = 0;
        for i in 0..self.focused_segment {
            if i < self.separators.len() {
                offset += self.separators[i].width();
            }
            offset += self.segments[i].display_string().width();
        }
        if self.focused_segment < self.separators.len() {
            offset += self.separators[self.focused_segment].width();
        }
        offset
    }
}
