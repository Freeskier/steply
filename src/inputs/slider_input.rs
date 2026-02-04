use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::style::{Color, Style};
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;

pub struct SliderInput {
    base: InputBase,
    min: i64,
    max: i64,
    step: i64,
    value: i64,
    track_len: usize,
    unit: Option<String>,
}

impl SliderInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, min: i64, max: i64) -> Self {
        let min_val = min.min(max);
        let max_val = min.max(max);
        Self {
            base: InputBase::new(id, label),
            min: min_val,
            max: max_val,
            step: 1,
            value: min_val,
            track_len: 15,
            unit: None,
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

    pub fn with_step(mut self, step: i64) -> Self {
        self.step = step.max(1);
        self
    }

    pub fn with_track_len(mut self, len: usize) -> Self {
        self.track_len = len.max(3);
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    fn clamp_value(&mut self) {
        if self.value < self.min {
            self.value = self.min;
        } else if self.value > self.max {
            self.value = self.max;
        }
    }

    fn shift_value(&mut self, delta: i64) {
        self.value += delta;
        self.clamp_value();
        self.base.error = None;
    }

    fn slider_position(&self) -> usize {
        if self.max == self.min {
            return 0;
        }
        let range = (self.max - self.min) as f64;
        let rel = (self.value - self.min) as f64 / range;
        let pos = (rel * (self.track_len as f64 - 1.0)).round();
        pos.clamp(0.0, (self.track_len - 1) as f64) as usize
    }
}

impl Input for SliderInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        self.value.to_string()
    }

    fn set_value(&mut self, value: String) {
        if let Ok(val) = value.parse::<i64>() {
            self.value = val;
            self.clamp_value();
        }
    }

    fn raw_value(&self) -> String {
        self.value.to_string()
    }

    fn is_complete(&self) -> bool {
        true
    }

    fn cursor_pos(&self) -> usize {
        0
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Left => {
                self.shift_value(-self.step);
                KeyResult::Handled
            }
            KeyCode::Right => {
                self.shift_value(self.step);
                KeyResult::Handled
            }
            KeyCode::Home => {
                self.value = self.min;
                KeyResult::Handled
            }
            KeyCode::End => {
                self.value = self.max;
                KeyResult::Handled
            }
            KeyCode::Enter => KeyResult::Submit,
            _ => KeyResult::NotHandled,
        }
    }

    fn render_content(&self, theme: &crate::theme::Theme) -> Vec<Span> {
        let value = self.value.to_string();
        let pos = self.slider_position();
        let left = '‹';
        let right = '›';
        let line = '—';
        let knob = '◈';
        let active_style = Style::new().with_color(Color::Green);

        let mut spans = Vec::new();
        spans.push(Span::new(left.to_string()));
        for i in 0..self.track_len {
            let ch = if i == pos { knob } else { line };
            if i <= pos {
                spans.push(Span::new(ch.to_string()).with_style(active_style.clone()));
            } else {
                spans.push(Span::new(ch.to_string()));
            }
        }
        spans.push(Span::new(right.to_string()));
        spans.push(Span::new(" "));
        spans.push(Span::new(value));
        if let Some(unit) = &self.unit {
            let mut style = Style::default();
            style = style.merge(&theme.placeholder);
            spans.push(Span::new(" "));
            spans.push(Span::new(unit.clone()).with_style(style));
        }
        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let pos = self.slider_position();
        1 + pos
    }
}
