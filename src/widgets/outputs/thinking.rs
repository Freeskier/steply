use std::time::{Duration, Instant};

use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, InteractionResult, OutputNode, RenderContext};

pub struct ThinkingOutput {
    id: String,
    label: String,
    text: String,
    chars: Vec<char>,
    frame: usize,
    tail_len: usize,
    tick_interval: Duration,
    last_tick: Instant,
    base_rgb: (u8, u8, u8),
    peak_rgb: (u8, u8, u8),
}

impl ThinkingOutput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            id: id.into(),
            label: label.into(),
            chars: text.chars().collect(),
            text,
            frame: 0,
            tail_len: 6,
            tick_interval: Duration::from_millis(70),
            last_tick: Instant::now(),
            base_rgb: (70, 78, 92),
            peak_rgb: (228, 236, 252),
        }
    }

    pub fn with_tail_len(mut self, tail_len: usize) -> Self {
        self.tail_len = tail_len.max(2);
        self
    }

    pub fn with_tick_ms(mut self, tick_ms: u64) -> Self {
        self.tick_interval = Duration::from_millis(tick_ms.max(16));
        self
    }

    pub fn with_gradient_rgb(mut self, base_rgb: (u8, u8, u8), peak_rgb: (u8, u8, u8)) -> Self {
        self.base_rgb = base_rgb;
        self.peak_rgb = peak_rgb;
        self
    }

    fn color_for_index(&self, idx: usize) -> (u8, u8, u8) {
        let len = self.chars.len();
        if len == 0 {
            return self.base_rgb;
        }

        let distance = (self.frame + len - (idx % len)) % len;
        if distance >= self.tail_len {
            return self.base_rgb;
        }

        let max = (self.tail_len - 1) as f32;
        let t = 1.0 - (distance as f32 / max);
        lerp_rgb(self.base_rgb, self.peak_rgb, t)
    }
}

impl Drawable for ThinkingOutput {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn label(&self) -> &str {
        self.label.as_str()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let mut lines = Vec::<Vec<Span>>::new();
        if !self.label.is_empty() {
            lines.push(vec![Span::new(self.label.clone()).no_wrap()]);
        }

        if self.chars.is_empty() {
            lines.push(vec![Span::new(self.text.clone()).no_wrap()]);
            return DrawOutput { lines };
        }

        let mut text_line = Vec::<Span>::with_capacity(self.chars.len());
        for (idx, ch) in self.chars.iter().enumerate() {
            let (r, g, b) = self.color_for_index(idx);
            let style = Style::new().color(Color::Rgb(r, g, b));
            text_line.push(Span::styled(ch.to_string(), style).no_wrap());
        }
        lines.push(text_line);
        DrawOutput { lines }
    }
}

impl OutputNode for ThinkingOutput {
    fn on_tick(&mut self) -> InteractionResult {
        if self.chars.is_empty() {
            return InteractionResult::ignored();
        }
        let now = Instant::now();
        if now.duration_since(self.last_tick) < self.tick_interval {
            return InteractionResult::ignored();
        }
        self.last_tick = now;
        self.frame = (self.frame + 1) % self.chars.len();
        InteractionResult::handled()
    }
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        lerp_channel(a.0, b.0, t),
        lerp_channel(a.1, b.1, t),
        lerp_channel(a.2, b.2, t),
    )
}
