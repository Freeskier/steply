use crate::core::value::Value;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, InteractionResult, OutputNode, RenderContext};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    OutQuad,
    OutCubic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressTransition {
    Immediate,
    Tween { duration_ms: u64, easing: Easing },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStyle {
    ClassicLine,
    BlockClassic,
}

#[derive(Debug, Clone, Copy)]
struct ProgressAnimation {
    from: f64,
    to: f64,
    started_at: Instant,
    duration: Duration,
    easing: Easing,
}

pub struct ProgressOutput {
    id: String,
    label: String,
    min: f64,
    max: f64,
    unit: Option<String>,
    bar_width: usize,
    target_value: f64,
    display_value: f64,
    transition: ProgressTransition,
    animation: Option<ProgressAnimation>,
    style: ProgressStyle,
}

impl ProgressOutput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            min: 0.0,
            max: 100.0,
            unit: None,
            bar_width: 30,
            target_value: 0.0,
            display_value: 0.0,
            transition: ProgressTransition::Tween {
                duration_ms: 350,
                easing: Easing::OutCubic,
            },
            animation: None,
            style: ProgressStyle::ClassicLine,
        }
    }

    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = min;
        self.max = if (max - min).abs() < f64::EPSILON {
            min + 1.0
        } else {
            max
        };
        self.target_value = self.clamp(self.target_value);
        self.display_value = self.clamp(self.display_value);
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn with_bar_width(mut self, width: usize) -> Self {
        self.bar_width = width.max(8);
        self
    }

    pub fn with_transition(mut self, transition: ProgressTransition) -> Self {
        self.transition = transition;
        if matches!(self.transition, ProgressTransition::Immediate) {
            self.display_value = self.target_value;
            self.animation = None;
        }
        self
    }

    pub fn with_style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    fn ratio(&self, value: f64) -> f64 {
        let range = (self.max - self.min).max(f64::EPSILON);
        ((value - self.min) / range).clamp(0.0, 1.0)
    }

    fn filled_cells(&self) -> usize {
        (self.ratio(self.display_value) * self.bar_width as f64).round() as usize
    }

    fn value_color(&self) -> Color {
        let ratio = self.ratio(self.display_value);
        if ratio < 0.6 {
            Color::Green
        } else if ratio < 0.85 {
            Color::Yellow
        } else {
            Color::Red
        }
    }

    fn formatted_value(&self) -> String {
        match &self.unit {
            Some(unit) => format!("{:.1}{unit}", self.display_value),
            None => format!("{:.1}", self.display_value),
        }
    }

    fn set_target(&mut self, target: f64) {
        let target = self.clamp(target);
        self.target_value = target;




        if let Some(animation) = self.animation {
            let elapsed = animation.started_at.elapsed();
            let duration = animation.duration.as_secs_f64().max(f64::EPSILON);
            let t = (elapsed.as_secs_f64() / duration).clamp(0.0, 1.0);
            let eased = apply_easing(t, animation.easing);
            self.display_value = animation.from + (animation.to - animation.from) * eased;
            if t >= 1.0 {
                self.animation = None;
            }
        }

        if (self.display_value - target).abs() < f64::EPSILON {
            self.animation = None;
            return;
        }

        match self.transition {
            ProgressTransition::Immediate => {
                self.display_value = target;
                self.animation = None;
            }
            ProgressTransition::Tween { .. } => {
                self.animation = self.transition();
            }
        }
    }

    fn transition(&self) -> Option<ProgressAnimation> {
        let ProgressTransition::Tween {
            duration_ms,
            easing,
        } = self.transition
        else {
            return None;
        };
        Some(ProgressAnimation {
            from: self.display_value,
            to: self.target_value,
            started_at: Instant::now(),
            duration: Duration::from_millis(duration_ms.max(1)),
            easing,
        })
    }

    fn glyphs(&self) -> (char, char) {
        match self.style {
            ProgressStyle::ClassicLine => ('▬', '─'),
            ProgressStyle::BlockClassic => ('▰', '▱'),
        }
    }
}

impl Drawable for ProgressOutput {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let mut lines = Vec::new();
        lines.push(vec![Span::new(self.label.clone()).no_wrap()]);

        let filled = self.filled_cells().min(self.bar_width);
        let empty = self.bar_width.saturating_sub(filled);
        let percent = self.ratio(self.display_value) * 100.0;
        let value_color = self.value_color();
        let (filled_glyph, empty_glyph) = self.glyphs();

        lines.push(vec![
            Span::new("[").no_wrap(),
            Span::styled(
                filled_glyph.to_string().repeat(filled),
                Style::new().color(value_color).bold(),
            )
            .no_wrap(),
            Span::styled(
                empty_glyph.to_string().repeat(empty),
                Style::new().color(Color::DarkGrey),
            )
            .no_wrap(),
            Span::new("] ").no_wrap(),
            Span::styled(
                format!("{percent:>5.1}%"),
                Style::new().color(value_color).bold(),
            )
            .no_wrap(),
            Span::new("  ").no_wrap(),
            Span::styled(
                format!("target {:.1}", self.target_value),
                Style::new().color(Color::DarkGrey),
            )
            .no_wrap(),
            Span::new("  ").no_wrap(),
            Span::styled(
                self.formatted_value(),
                Style::new().color(Color::White).bold(),
            )
            .no_wrap(),
        ]);

        DrawOutput { lines }
    }
}

impl OutputNode for ProgressOutput {
    fn set_value(&mut self, value: Value) {
        if let Some(number) = value.to_number() {
            self.set_target(number);
            return;
        }
        if let Some(last) = value.list_last()
            && let Some(number) = last.to_number()
        {
            self.set_target(number);
            return;
        }
        if matches!(value, Value::None) {
            self.set_target(self.min);
        }
    }

    fn on_tick(&mut self) -> InteractionResult {
        if matches!(self.transition, ProgressTransition::Immediate) {
            return InteractionResult::ignored();
        }

        let Some(animation) = self.animation else {
            return InteractionResult::ignored();
        };

        let elapsed = animation.started_at.elapsed();
        let duration = animation.duration.as_secs_f64().max(f64::EPSILON);
        let t = (elapsed.as_secs_f64() / duration).clamp(0.0, 1.0);
        let eased = apply_easing(t, animation.easing);
        let next = animation.from + (animation.to - animation.from) * eased;
        self.display_value = if t >= 1.0 { animation.to } else { next };
        if t >= 1.0 {
            self.animation = None;
        }
        InteractionResult::handled()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Number(self.target_value))
    }
}

fn apply_easing(t: f64, easing: Easing) -> f64 {
    match easing {
        Easing::Linear => t,
        Easing::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
    }
}
