use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::{ValueChange, WidgetAction};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct SliderInput {
    base: WidgetBase,
    min: i64,
    max: i64,
    step: i64,
    value: i64,
    track_len: usize,
    unit: Option<String>,
    change_target: Option<ValueTarget>,
    validators: Vec<Validator>,
}

impl SliderInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, min: i64, max: i64) -> Self {
        let min_value = min.min(max);
        let max_value = min.max(max);
        Self {
            base: WidgetBase::new(id, label),
            min: min_value,
            max: max_value,
            step: 1,
            value: min_value,
            track_len: 15,
            unit: None,
            change_target: None,
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

    pub fn with_change_target(mut self, target: impl Into<NodeId>) -> Self {
        self.change_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_change_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.change_target = Some(ValueTarget::path(root, path));
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

    fn clamp_value(&mut self) {
        self.value = self.value.clamp(self.min, self.max);
    }

    fn shift(&mut self, delta: i64) {
        self.value = self.value.saturating_add(delta);
        self.clamp_value();
    }

    fn value_changed_result(&self, previous: i64) -> InteractionResult {
        if self.value == previous {
            return InteractionResult::ignored();
        }
        if let Some(target) = &self.change_target {
            return InteractionResult::with_action(WidgetAction::ValueChanged {
                change: ValueChange::with_target(target.clone(), Value::Number(self.value as f64)),
            });
        }
        InteractionResult::handled()
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

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let spans = if focused {
            let knob_position = self.track_position();
            let active_track_style = Style::new().color(Color::Green);
            let mut s = vec![Span::new("‹").no_wrap()];
            for idx in 0..self.track_len {
                let symbol = if idx == knob_position { '◈' } else { '—' };
                s.push(if idx <= knob_position {
                    Span::styled(symbol.to_string(), active_track_style).no_wrap()
                } else {
                    Span::new(symbol.to_string()).no_wrap()
                });
            }
            s.push(Span::new("› ").no_wrap());
            s.push(Span::styled(self.value.to_string(), Style::default()).no_wrap());
            if let Some(unit) = &self.unit {
                s.push(Span::new(" ").no_wrap());
                s.push(Span::styled(unit.clone(), Style::new().color(Color::DarkGrey)).no_wrap());
            }
            s
        } else {
            let mut s = vec![Span::new(self.value.to_string()).no_wrap()];
            if let Some(unit) = &self.unit {
                s.push(Span::new(" ").no_wrap());
                s.push(Span::styled(unit.clone(), Style::new().color(Color::DarkGrey)).no_wrap());
            }
            s
        };

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
                let previous = self.value;
                self.shift(-self.step);
                self.value_changed_result(previous)
            }
            KeyCode::Right => {
                let previous = self.value;
                self.shift(self.step);
                self.value_changed_result(previous)
            }
            KeyCode::Home => {
                let previous = self.value;
                self.value = self.min;
                self.value_changed_result(previous)
            }
            KeyCode::End => {
                let previous = self.value;
                self.value = self.max;
                self.value_changed_result(previous)
            }
            KeyCode::Enter => InteractionResult::input_done(),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Number(self.value as f64))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(number) = value.to_number() {
            self.value = number.round() as i64;
            self.clamp_value();
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &Value::Number(self.value as f64))
    }
}
