use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use crate::widgets::validators::Validator;

pub struct SliderInput {
    base: InputBase,
    min: i64,
    max: i64,
    step: i64,
    value: i64,
    track_len: usize,
    unit: Option<String>,
    validators: Vec<Validator>,
}

impl SliderInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, min: i64, max: i64) -> Self {
        let min_value = min.min(max);
        let max_value = min.max(max);
        Self {
            base: InputBase::new(id, label),
            min: min_value,
            max: max_value,
            step: 1,
            value: min_value,
            track_len: 15,
            unit: None,
            validators: Vec::new(),
        }
    }

    pub fn with_step(mut self, step: i64) -> Self {
        self.step = step.max(1);
        self
    }

    pub fn with_track_len(mut self, track_len: usize) -> Self {
        self.track_len = track_len.max(3);
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn clamp_value(&mut self) {
        if self.value < self.min {
            self.value = self.min;
        } else if self.value > self.max {
            self.value = self.max;
        }
    }

    fn shift(&mut self, delta: i64) {
        self.value = self.value.saturating_add(delta);
        self.clamp_value();
    }

    fn track_position(&self) -> usize {
        if self.max == self.min {
            return 0;
        }

        let range = (self.max - self.min) as f64;
        let ratio = (self.value - self.min) as f64 / range;
        let raw = (ratio * (self.track_len as f64 - 1.0)).round();
        raw.clamp(0.0, (self.track_len - 1) as f64) as usize
    }
}

impl Drawable for SliderInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line = self.base.line_state(ctx);

        let value_style = Style::default();
        let active_track_style = Style::new().color(Color::Green);

        let knob_position = self.track_position();
        let mut spans = vec![Span::new(line.prefix).no_wrap(), Span::new("‹").no_wrap()];
        for idx in 0..self.track_len {
            let symbol = if idx == knob_position { '◈' } else { '—' };
            let span = if idx <= knob_position {
                Span::styled(symbol.to_string(), active_track_style).no_wrap()
            } else {
                Span::new(symbol.to_string()).no_wrap()
            };
            spans.push(span);
        }
        spans.push(Span::new("› ").no_wrap());
        spans.push(Span::styled(self.value.to_string(), value_style).no_wrap());
        if let Some(unit) = &self.unit {
            spans.push(Span::new(" ").no_wrap());
            spans.push(Span::styled(unit.clone(), Style::new().color(Color::DarkGrey)).no_wrap());
        }

        DrawOutput { lines: vec![spans] }
    }
}

impl Interactive for SliderInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Left => {
                self.shift(-self.step);
                InteractionResult::handled()
            }
            KeyCode::Right => {
                self.shift(self.step);
                InteractionResult::handled()
            }
            KeyCode::Home => {
                self.value = self.min;
                InteractionResult::handled()
            }
            KeyCode::End => {
                self.value = self.max;
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::submit_requested(),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Number(self.value))
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::Number(number) => {
                self.value = number;
                self.clamp_value();
            }
            Value::Text(text) => {
                if let Ok(number) = text.parse::<i64>() {
                    self.value = number;
                    self.clamp_value();
                }
            }
            _ => {}
        }
    }

    fn validate_submit(&self) -> Result<(), String> {
        let value_text = self.value.to_string();
        for validator in &self.validators {
            validator(value_text.as_str())?;
        }
        Ok(())
    }
}
