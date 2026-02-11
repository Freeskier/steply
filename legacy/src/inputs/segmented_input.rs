use super::text_edit;
use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::style::Style;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Digit,
    Alpha,
    Alnum,
    NumericRange { min: i64, max: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentRole {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentToken {
    Literal(char),
    Segment {
        kind: SegmentKind,
        min_len: usize,
        max_len: Option<usize>,
        role: Option<SegmentRole>,
        value: String,
    },
}

pub struct SegmentedInput {
    base: InputBase,
    tokens: Vec<SegmentToken>,
    cursor_token: usize,
    cursor_offset: usize,
}

impl SegmentedInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, mask: impl Into<String>) -> Self {
        let tokens = Self::parse_mask(mask.into().as_str());
        let cursor_token = Self::first_segment_pos(&tokens).unwrap_or(0);
        Self {
            base: InputBase::new(id, label),
            tokens,
            cursor_token,
            cursor_offset: 0,
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

    fn parse_mask(mask: &str) -> Vec<SegmentToken> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = mask.chars().collect();
        let mut idx = 0;
        while idx < chars.len() {
            let ch = chars[idx];
            let kind = match ch {
                '#' => Some(SegmentKind::Digit),
                'A' => Some(SegmentKind::Alpha),
                '*' => Some(SegmentKind::Alnum),
                _ => None,
            };

            if let Some(kind) = kind {
                let (min_len, max_len, range, new_idx) = Self::parse_quantifier(&chars, idx + 1);
                let seg_kind = if let Some((min, max)) = range {
                    SegmentKind::NumericRange { min, max }
                } else {
                    kind
                };
                tokens.push(SegmentToken::Segment {
                    kind: seg_kind,
                    min_len,
                    max_len,
                    role: None,
                    value: String::new(),
                });
                idx = new_idx;
                continue;
            }

            if ch.is_alphabetic() {
                let start = idx;
                while idx < chars.len() && chars[idx].is_alphabetic() {
                    idx += 1;
                }
                let token: String = chars[start..idx].iter().collect();
                if let Some((kind, min_len, max_len, role)) = Self::date_token(&token) {
                    tokens.push(SegmentToken::Segment {
                        kind,
                        min_len,
                        max_len,
                        role: Some(role),
                        value: String::new(),
                    });
                } else {
                    for c in token.chars() {
                        tokens.push(SegmentToken::Literal(c));
                    }
                }
                continue;
            }

            tokens.push(SegmentToken::Literal(ch));
            idx += 1;
        }
        tokens
    }

    fn date_token(token: &str) -> Option<(SegmentKind, usize, Option<usize>, SegmentRole)> {
        match token {
            "YYYY" => Some((
                SegmentKind::NumericRange {
                    min: 1900,
                    max: 2100,
                },
                4,
                Some(4),
                SegmentRole::Year,
            )),
            "MM" => Some((
                SegmentKind::NumericRange { min: 1, max: 12 },
                2,
                Some(2),
                SegmentRole::Month,
            )),
            "DD" => Some((
                SegmentKind::NumericRange { min: 1, max: 31 },
                2,
                Some(2),
                SegmentRole::Day,
            )),
            "HH" => Some((
                SegmentKind::NumericRange { min: 0, max: 23 },
                2,
                Some(2),
                SegmentRole::Hour,
            )),
            "mm" => Some((
                SegmentKind::NumericRange { min: 0, max: 59 },
                2,
                Some(2),
                SegmentRole::Minute,
            )),
            "ss" => Some((
                SegmentKind::NumericRange { min: 0, max: 59 },
                2,
                Some(2),
                SegmentRole::Second,
            )),
            _ => None,
        }
    }

    fn parse_quantifier(
        chars: &[char],
        mut idx: usize,
    ) -> (usize, Option<usize>, Option<(i64, i64)>, usize) {
        if idx >= chars.len() || chars[idx] != '{' {
            return (1, Some(1), None, idx);
        }
        idx += 1;
        let start = idx;
        while idx < chars.len() && chars[idx] != '}' {
            idx += 1;
        }
        if idx >= chars.len() || chars[idx] != '}' {
            return (1, Some(1), None, start - 1);
        }
        let inner: String = chars[start..idx].iter().collect();
        idx += 1;
        if inner.is_empty() {
            return (0, None, None, idx);
        }

        let mut len_part = inner.as_str();
        let mut range: Option<(i64, i64)> = None;
        if let Some((left, right)) = inner.split_once(':') {
            len_part = left;
            if let Some((rmin, rmax)) = right.split_once('-') {
                if let (Ok(min), Ok(max)) = (rmin.parse::<i64>(), rmax.parse::<i64>()) {
                    range = Some((min, max));
                }
            }
        }

        let (min_len, max_len) = if len_part.contains(',') {
            let parts: Vec<&str> = len_part.split(',').collect();
            let min = parts
                .get(0)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let max = parts.get(1).and_then(|v| v.parse::<usize>().ok());
            (min, max)
        } else {
            let len = len_part.parse::<usize>().unwrap_or(1);
            (len, Some(len))
        };

        (min_len, max_len, range, idx)
    }

    fn first_segment_pos(tokens: &[SegmentToken]) -> Option<usize> {
        tokens
            .iter()
            .position(|t| matches!(t, SegmentToken::Segment { .. }))
    }

    fn next_segment_pos(tokens: &[SegmentToken], from: usize) -> Option<usize> {
        let start = from + 1;
        tokens[start..]
            .iter()
            .position(|t| matches!(t, SegmentToken::Segment { .. }))
            .map(|offset| start + offset)
    }

    fn prev_segment_pos(tokens: &[SegmentToken], from: usize) -> Option<usize> {
        if from == 0 {
            return None;
        }
        tokens[..from]
            .iter()
            .rposition(|t| matches!(t, SegmentToken::Segment { .. }))
    }

    fn token_accepts(kind: SegmentKind, ch: char) -> bool {
        match kind {
            SegmentKind::Digit | SegmentKind::NumericRange { .. } => ch.is_ascii_digit(),
            SegmentKind::Alpha => ch.is_ascii_alphabetic(),
            SegmentKind::Alnum => ch.is_ascii_alphanumeric(),
        }
    }

    fn current_segment(&self) -> Option<&SegmentToken> {
        self.tokens.get(self.cursor_token)
    }

    fn insert_char(&mut self, ch: char) -> bool {
        let token_idx = self.cursor_token;
        let cursor_offset = self.cursor_offset;
        let next_segment = Self::next_segment_pos(&self.tokens, token_idx);
        let Some(SegmentToken::Segment {
            kind,
            min_len: _,
            max_len,
            value,
            ..
        }) = self.tokens.get_mut(token_idx)
        else {
            return false;
        };

        if !Self::token_accepts(*kind, ch) {
            return false;
        }

        if let Some(max) = max_len
            && text_edit::char_count(value) >= *max
        {
            value.clear();
        }

        let mut next_cursor = cursor_offset;
        text_edit::insert_char(value, &mut next_cursor, ch);
        self.base.error = None;

        let new_len = text_edit::char_count(value);
        if let Some(max) = max_len
            && new_len >= *max
        {
            if let Some(next) = next_segment {
                self.cursor_token = next;
                self.cursor_offset = 0;
            } else {
                self.cursor_offset = new_len.min(*max);
            }
            return true;
        }

        self.cursor_offset = next_cursor;
        true
    }

    fn delete_prev(&mut self) -> bool {
        let token_idx = self.cursor_token;
        let cursor_offset = self.cursor_offset;
        let Some(SegmentToken::Segment { value, .. }) = self.tokens.get_mut(token_idx) else {
            return false;
        };

        if cursor_offset > 0 && text_edit::backspace_char(value, &mut self.cursor_offset) {
            self.base.error = None;
            return true;
        }

        if let Some(prev) = Self::prev_segment_pos(&self.tokens, token_idx) {
            self.cursor_token = prev;
            self.cursor_offset = match self.current_segment() {
                Some(SegmentToken::Segment { value, .. }) => text_edit::char_count(value),
                _ => 0,
            };
            return self.delete_prev();
        }

        false
    }

    fn delete_current(&mut self) -> bool {
        let token_idx = self.cursor_token;
        let Some(SegmentToken::Segment { value, .. }) = self.tokens.get_mut(token_idx) else {
            return false;
        };
        if text_edit::delete_char(value, &mut self.cursor_offset) {
            self.base.error = None;
            return true;
        }
        false
    }

    fn move_left(&mut self) -> bool {
        if self.cursor_offset > 0 {
            self.cursor_offset -= 1;
            return true;
        }
        if let Some(prev) = Self::prev_segment_pos(&self.tokens, self.cursor_token) {
            self.cursor_token = prev;
            self.cursor_offset = match self.current_segment() {
                Some(SegmentToken::Segment { value, .. }) => text_edit::char_count(value),
                _ => 0,
            };
            return true;
        }
        false
    }

    fn move_right(&mut self) -> bool {
        if let Some(SegmentToken::Segment { value, .. }) = self.current_segment()
            && self.cursor_offset < text_edit::char_count(value)
        {
            self.cursor_offset += 1;
            return true;
        }
        if let Some(next) = Self::next_segment_pos(&self.tokens, self.cursor_token) {
            self.cursor_token = next;
            self.cursor_offset = 0;
            return true;
        }
        false
    }

    fn increment_current(&mut self, delta: i64) -> bool {
        let Some(SegmentToken::Segment {
            kind,
            min_len,
            max_len: _,
            value,
            ..
        }) = self.tokens.get_mut(self.cursor_token)
        else {
            return false;
        };

        let SegmentKind::NumericRange { min, max } = kind else {
            return false;
        };

        let current = value.parse::<i64>().unwrap_or(*min);
        let mut next = current + delta;
        if next > *max {
            next = *min;
        } else if next < *min {
            next = *max;
        }

        if *min_len > 0 {
            *value = format!("{:0width$}", next, width = *min_len);
        } else {
            *value = next.to_string();
        }
        self.cursor_offset = text_edit::char_count(value);
        true
    }

    fn formatted_value(&self, placeholder: Option<char>) -> String {
        let mut out = String::new();
        for token in &self.tokens {
            match token {
                SegmentToken::Literal(ch) => out.push(*ch),
                SegmentToken::Segment {
                    value,
                    max_len,
                    min_len,
                    ..
                } => {
                    if value.is_empty() {
                        if let Some(ph) = placeholder {
                            if let Some(max) = max_len {
                                if min_len == max {
                                    for _ in 0..*max {
                                        out.push(ph);
                                    }
                                } else {
                                    out.push(ph);
                                }
                            }
                        }
                    } else {
                        out.push_str(value);
                        if let Some(ph) = placeholder {
                            if let Some(max) = max_len {
                                if min_len == max {
                                    let pad = max.saturating_sub(text_edit::char_count(value));
                                    for _ in 0..pad {
                                        out.push(ph);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn is_complete_internal(&self) -> bool {
        let mut year: Option<i64> = None;
        let mut month: Option<i64> = None;
        let mut day: Option<i64> = None;

        let ok = self.tokens.iter().all(|token| match token {
            SegmentToken::Literal(_) => true,
            SegmentToken::Segment {
                kind,
                min_len,
                value,
                role,
                ..
            } => {
                if text_edit::char_count(value) < *min_len {
                    return false;
                }
                if let SegmentKind::NumericRange { min, max } = kind {
                    let num = value.parse::<i64>().unwrap_or(*min);
                    if num < *min || num > *max {
                        return false;
                    }
                }
                if let Some(role) = role {
                    let num = value.parse::<i64>().unwrap_or(0);
                    match role {
                        SegmentRole::Year => year = Some(num),
                        SegmentRole::Month => month = Some(num),
                        SegmentRole::Day => day = Some(num),
                        _ => {}
                    }
                }
                true
            }
        });

        if !ok {
            return false;
        }

        if let (Some(m), Some(d)) = (month, day) {
            let max_day = Self::days_in_month(year, m);
            if max_day == 0 || d < 1 || d > max_day {
                return false;
            }
        }

        true
    }
}

impl SegmentedInput {
    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    fn days_in_month(year: Option<i64>, month: i64) -> i64 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => match year {
                Some(y) if Self::is_leap_year(y) => 29,
                Some(_) => 28,
                None => 29,
            },
            _ => 0,
        }
    }

    fn role_placeholder(role: SegmentRole) -> &'static str {
        match role {
            SegmentRole::Year => "yyyy",
            SegmentRole::Month => "mm",
            SegmentRole::Day => "dd",
            SegmentRole::Hour => "hh",
            SegmentRole::Minute => "mm",
            SegmentRole::Second => "ss",
        }
    }

    fn segment_display_len(token: &SegmentToken) -> usize {
        match token {
            SegmentToken::Literal(_) => 1,
            SegmentToken::Segment {
                value,
                max_len,
                min_len,
                role,
                ..
            } => {
                if role.is_some() {
                    (*max_len).unwrap_or(text_edit::char_count(value).max(1))
                } else if let Some(max) = max_len {
                    if min_len == max {
                        *max
                    } else if value.is_empty() {
                        1
                    } else {
                        text_edit::char_count(value)
                    }
                } else if value.is_empty() {
                    1
                } else {
                    text_edit::char_count(value)
                }
            }
        }
    }
}

impl Input for SegmentedInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        if self.is_complete_internal() {
            self.formatted_value(None)
        } else {
            String::new()
        }
    }

    fn set_value(&mut self, value: String) {
        let chars: Vec<char> = value.chars().collect();
        let mut idx = 0usize;
        let next_literals: Vec<Option<char>> = (0..self.tokens.len())
            .map(|i| self.next_literal_char(i))
            .collect();
        for (token_idx, token) in self.tokens.iter_mut().enumerate() {
            match token {
                SegmentToken::Literal(ch) => {
                    if chars.get(idx).copied() != Some(*ch) {
                        return;
                    }
                    idx += 1;
                }
                SegmentToken::Segment {
                    kind,
                    max_len,
                    value: buf,
                    ..
                } => {
                    buf.clear();
                    let next_literal = next_literals[token_idx];
                    while idx < chars.len() {
                        let ch = chars[idx];
                        if let Some(next_lit) = next_literal {
                            if ch == next_lit {
                                break;
                            }
                        }
                        if let Some(max) = max_len
                            && text_edit::char_count(buf) >= *max
                        {
                            break;
                        }
                        if Self::token_accepts(*kind, ch) {
                            buf.push(ch);
                            idx += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn value_typed(&self) -> crate::value::Value {
        let mut items = Vec::new();
        let mut seg_idx = 1usize;
        let mut any_non_empty = false;

        for token in &self.tokens {
            if let SegmentToken::Segment { value, .. } = token {
                if !value.is_empty() {
                    any_non_empty = true;
                }
                items.push((format!("segment_{}", seg_idx), value.clone()));
                seg_idx += 1;
            }
        }

        if !any_non_empty {
            crate::value::Value::None
        } else {
            crate::value::Value::Map(items)
        }
    }

    fn set_value_typed(&mut self, value: crate::value::Value) {
        match value {
            crate::value::Value::Map(items) => {
                let mut map = std::collections::HashMap::new();
                for (k, v) in items {
                    map.insert(k, v);
                }

                let mut seg_idx = 1usize;
                for token in &mut self.tokens {
                    if let SegmentToken::Segment {
                        kind,
                        max_len,
                        value: buf,
                        ..
                    } = token
                    {
                        let key = format!("segment_{}", seg_idx);
                        let raw = map.get(&key).cloned().unwrap_or_default();
                        buf.clear();
                        for ch in raw.chars() {
                            if let Some(max) = max_len
                                && text_edit::char_count(buf) >= *max
                            {
                                break;
                            }
                            if Self::token_accepts(*kind, ch) {
                                buf.push(ch);
                            }
                        }
                        seg_idx += 1;
                    }
                }
            }
            crate::value::Value::Text(text) => {
                self.set_value(text);
            }
            crate::value::Value::None => {
                for token in &mut self.tokens {
                    if let SegmentToken::Segment { value: buf, .. } = token {
                        buf.clear();
                    }
                }
            }
            _ => {}
        }
    }

    fn raw_value(&self) -> String {
        if self.tokens.iter().all(|token| {
            matches!(token, SegmentToken::Segment { value, .. } if value.is_empty())
                || matches!(token, SegmentToken::Literal(_))
        }) {
            String::new()
        } else {
            self.formatted_value(None)
        }
    }

    fn is_complete(&self) -> bool {
        self.is_complete_internal()
    }

    fn cursor_pos(&self) -> usize {
        self.cursor_token
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(ch) => {
                if self.insert_char(ch) {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Backspace => {
                if self.delete_prev() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Delete => {
                if self.delete_current() {
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
            KeyCode::Up => {
                if self.increment_current(1) {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Down => {
                if self.increment_current(-1) {
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
        for token in &self.tokens {
            match token {
                SegmentToken::Literal(ch) => {
                    spans.push(Span::new(ch.to_string()));
                }
                SegmentToken::Segment {
                    value,
                    max_len,
                    min_len,
                    role,
                    ..
                } => {
                    if let Some(role) = role {
                        let placeholder = Self::role_placeholder(*role);
                        let max = max_len.unwrap_or(text_edit::char_count(placeholder));
                        let mut out: Vec<char> = value.chars().collect();
                        let ph_chars: Vec<char> = placeholder.chars().collect();
                        for idx in out.len()..max {
                            let ph = ph_chars.get(idx).copied().unwrap_or('_');
                            out.push(ph);
                        }
                        for (idx, ch) in out.into_iter().enumerate() {
                            if idx < text_edit::char_count(value) {
                                spans.push(Span::new(ch.to_string()));
                            } else {
                                let mut style = Style::default();
                                style = style.merge(&theme.placeholder);
                                spans.push(Span::new(ch.to_string()).with_style(style));
                            }
                        }
                    } else {
                        let used = text_edit::char_count(value);
                        for ch in value.chars() {
                            spans.push(Span::new(ch.to_string()));
                        }
                        if let Some(max) = max_len {
                            if min_len == max {
                                let pad = max.saturating_sub(used);
                                for _ in 0..pad {
                                    let mut style = Style::default();
                                    style = style.merge(&theme.placeholder);
                                    spans.push(Span::new("_").with_style(style));
                                }
                            } else if used == 0 {
                                let mut style = Style::default();
                                style = style.merge(&theme.placeholder);
                                spans.push(Span::new("_").with_style(style));
                            }
                        } else if used == 0 {
                            let mut style = Style::default();
                            style = style.merge(&theme.placeholder);
                            spans.push(Span::new("_").with_style(style));
                        }
                    }
                }
            }
        }
        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let mut offset = 0usize;
        for (idx, token) in self.tokens.iter().enumerate() {
            match token {
                SegmentToken::Literal(_) => {
                    offset += 1;
                }
                SegmentToken::Segment { .. } => {
                    if idx == self.cursor_token {
                        offset += self.cursor_offset;
                        return offset;
                    }
                    offset += Self::segment_display_len(token);
                }
            }
        }
        offset
    }
}

impl SegmentedInput {
    fn next_literal_char(&self, current_token: usize) -> Option<char> {
        let start = current_token + 1;
        self.tokens[start..].iter().find_map(|token| match token {
            SegmentToken::Literal(ch) => Some(*ch),
            _ => None,
        })
    }
}
