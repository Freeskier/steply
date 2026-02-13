use crate::core::value::Value;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, RenderContext, RenderNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartRenderMode {
    Braille,
    Dots,
    Sparkline,
}

pub struct ChartOutput {
    id: String,
    label: String,
    mode: ChartRenderMode,
    points: Vec<f64>,
    capacity: usize,
    fixed_min: Option<f64>,
    fixed_max: Option<f64>,
    unit: Option<String>,
    gradient: bool,
}

impl ChartOutput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            mode: ChartRenderMode::Braille,
            points: Vec::new(),
            capacity: 80,
            fixed_min: None,
            fixed_max: None,
            unit: None,
            gradient: false,
        }
    }

    pub fn with_mode(mut self, mode: ChartRenderMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity.max(4);
        self
    }

    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.fixed_min = Some(min);
        self.fixed_max = Some(max);
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn with_gradient(mut self, enabled: bool) -> Self {
        self.gradient = enabled;
        self
    }

    fn push_point(&mut self, value: f64) {
        if self.capacity == 0 {
            return;
        }
        if self.points.len() >= self.capacity {
            self.points.remove(0);
        }
        self.points.push(value);
    }

    fn append_points_from_list(&mut self, items: &[String]) {
        for item in items {
            if let Some(value) = parse_number(item.as_str()) {
                self.push_point(value);
            }
        }
    }

    fn unit_suffix(&self) -> &str {
        self.unit.as_deref().unwrap_or("")
    }

    fn value_range(&self) -> Option<(f64, f64)> {
        if let (Some(min), Some(max)) = (self.fixed_min, self.fixed_max) {
            let max = if max <= min { min + 1.0 } else { max };
            return Some((min, max));
        }

        let mut min = *self.points.first()?;
        let mut max = min;
        for point in &self.points {
            if *point < min {
                min = *point;
            }
            if *point > max {
                max = *point;
            }
        }
        if (max - min).abs() < f64::EPSILON {
            Some((min - 1.0, max + 1.0))
        } else {
            Some((min, max))
        }
    }

    fn normalized_points(&self, min: f64, max: f64) -> Vec<f64> {
        let range = (max - min).max(f64::EPSILON);
        self.points
            .iter()
            .map(|point| ((point - min) / range).clamp(0.0, 1.0))
            .collect()
    }

    fn render_series(&self, normalized: &[f64]) -> Vec<(char, f64)> {
        match self.mode {
            ChartRenderMode::Braille => render_braille(normalized),
            ChartRenderMode::Dots => render_dots(normalized),
            ChartRenderMode::Sparkline => render_sparkline(normalized),
        }
    }

    fn render_series_line(&self, series: &[(char, f64)]) -> Vec<Span> {
        if !self.gradient {
            let text = series.iter().map(|(ch, _)| *ch).collect::<String>();
            return vec![Span::styled(text, Style::new().color(Color::Cyan).bold()).no_wrap()];
        }

        let mut spans = Vec::<Span>::new();
        let mut buffer = String::new();
        let mut current_color: Option<Color> = None;

        for (ch, level) in series {
            let color = gradient_color(*level);
            if current_color != Some(color) {
                if let Some(prev_color) = current_color {
                    spans.push(
                        Span::styled(buffer.clone(), Style::new().color(prev_color)).no_wrap(),
                    );
                    buffer.clear();
                }
                current_color = Some(color);
            }
            buffer.push(*ch);
        }

        if let Some(color) = current_color
            && !buffer.is_empty()
        {
            spans.push(Span::styled(buffer, Style::new().color(color)).no_wrap());
        }

        spans
    }
}

impl Drawable for ChartOutput {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let mut lines = Vec::new();
        lines.push(vec![Span::new(self.label.clone()).no_wrap()]);

        if self.points.is_empty() {
            lines.push(vec![
                Span::styled("No data yet", Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
            return DrawOutput { lines };
        }

        let Some((min, max)) = self.value_range() else {
            return DrawOutput { lines };
        };
        let normalized = self.normalized_points(min, max);
        let series = self.render_series(normalized.as_slice());

        let now = *self.points.last().unwrap_or(&0.0);
        let avg = self.points.iter().sum::<f64>() / self.points.len() as f64;
        let mut min_seen = now;
        let mut max_seen = now;
        for point in &self.points {
            if *point < min_seen {
                min_seen = *point;
            }
            if *point > max_seen {
                max_seen = *point;
            }
        }

        let unit = self.unit_suffix();
        lines.push(vec![
            Span::styled(
                format!(
                    "now: {:.1}{}   avg: {:.1}{}   min: {:.1}{}   max: {:.1}{}",
                    now, unit, avg, unit, min_seen, unit, max_seen, unit
                ),
                Style::new().color(Color::DarkGrey),
            )
            .no_wrap(),
        ]);
        lines.push(self.render_series_line(series.as_slice()));

        DrawOutput { lines }
    }
}

impl RenderNode for ChartOutput {
    fn set_value(&mut self, value: Value) {
        match value {
            Value::Float(number) => self.push_point(number),
            Value::Text(text) => {
                if let Some(value) = parse_number(text.as_str()) {
                    self.push_point(value);
                }
            }
            Value::List(values) => self.append_points_from_list(values.as_slice()),
            Value::None => self.points.clear(),
            Value::Bool(flag) => self.push_point(if flag { 1.0 } else { 0.0 }),
        }
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = trimmed.parse::<f64>() {
        return Some(value);
    }

    let first = trimmed
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches('%');
    first.parse::<f64>().ok()
}

fn render_sparkline(points: &[f64]) -> Vec<(char, f64)> {
    const CHARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if points.is_empty() {
        return Vec::new();
    }
    points
        .iter()
        .map(|value| {
            let ch = quantize(value, CHARS.len())
                .map(|idx| CHARS[idx])
                .unwrap_or(' ');
            (ch, *value)
        })
        .collect()
}

fn render_dots(points: &[f64]) -> Vec<(char, f64)> {
    const CHARS: &[char] = &['·', '•', '◉', '●'];
    if points.is_empty() {
        return Vec::new();
    }
    points
        .iter()
        .map(|value| {
            let ch = quantize(value, CHARS.len())
                .map(|idx| CHARS[idx])
                .unwrap_or(' ');
            (ch, *value)
        })
        .collect()
}

fn render_braille(points: &[f64]) -> Vec<(char, f64)> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(points.len().div_ceil(2));
    let mut idx = 0usize;
    while idx < points.len() {
        let left_value = points[idx];
        let left = level_0_to_4(left_value);
        let right = if idx + 1 < points.len() {
            level_0_to_4(points[idx + 1])
        } else {
            0
        };
        let right_value = if idx + 1 < points.len() {
            points[idx + 1]
        } else {
            0.0
        };
        let mask = column_mask(left, true) | column_mask(right, false);
        let ch = char::from_u32(0x2800 + mask as u32).unwrap_or('⣿');
        out.push((ch, left_value.max(right_value)));
        idx += 2;
    }
    out
}

fn gradient_color(level: f64) -> Color {
    if level < 0.2 {
        Color::Rgb(110, 116, 124)
    } else if level < 0.4 {
        Color::Rgb(77, 128, 198)
    } else if level < 0.6 {
        Color::Rgb(74, 179, 209)
    } else if level < 0.8 {
        Color::Rgb(218, 173, 90)
    } else {
        Color::Rgb(214, 98, 90)
    }
}

fn quantize(value: &f64, buckets: usize) -> Option<usize> {
    if buckets == 0 {
        return None;
    }
    let scaled = (value.clamp(0.0, 1.0) * ((buckets - 1) as f64)).round() as usize;
    Some(scaled.min(buckets - 1))
}

fn level_0_to_4(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 4.0).round() as u8
}

fn column_mask(level: u8, left: bool) -> u8 {
    let level = level.min(4) as usize;
    if level == 0 {
        return 0;
    }

    let bits_left: [u8; 4] = [0x40, 0x04, 0x02, 0x01];
    let bits_right: [u8; 4] = [0x80, 0x20, 0x10, 0x08];
    let bits = if left { bits_left } else { bits_right };

    let mut mask = 0u8;
    for bit in bits.iter().take(level) {
        mask |= *bit;
    }
    mask
}
