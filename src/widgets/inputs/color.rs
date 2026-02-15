use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use unicode_width::UnicodeWidthStr;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Hex,
    Rgb,
    Hsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    First = 0,
    Second = 1,
    Third = 2,
}

impl Channel {
    fn next(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::Third,
            Self::Third => Self::First,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::First => Self::Third,
            Self::Second => Self::First,
            Self::Third => Self::Second,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

pub struct ColorInput {
    base: WidgetBase,
    rgb: [u8; 3],
    mode: ColorMode,
    channel: Channel,
    edit_buffer: String,
    edit_mode: ColorMode,
    edit_channel: Channel,
    submit_target: Option<String>,
    validators: Vec<Validator>,
}

impl ColorInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            rgb: [0, 0, 0],
            mode: ColorMode::Hex,
            channel: Channel::First,
            edit_buffer: String::new(),
            edit_mode: ColorMode::Hex,
            edit_channel: Channel::First,
            submit_target: None,
            validators: Vec::new(),
        }
    }

    pub fn with_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.rgb = [r, g, b];
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn reset_edit_buffer(&mut self) {
        self.edit_buffer.clear();
        self.edit_mode = self.mode;
        self.edit_channel = self.channel;
    }

    fn ensure_edit_buffer(&mut self) {
        if self.edit_mode != self.mode || self.edit_channel != self.channel {
            self.reset_edit_buffer();
        }
    }

    fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            ColorMode::Hex => ColorMode::Rgb,
            ColorMode::Rgb => ColorMode::Hsl,
            ColorMode::Hsl => ColorMode::Hex,
        };
        self.reset_edit_buffer();
    }

    fn adjust_channel(&mut self, delta: i32) {
        match self.mode {
            ColorMode::Hex | ColorMode::Rgb => {
                let idx = self.channel.index();
                let value = self.rgb[idx] as i32 + delta;
                self.rgb[idx] = value.clamp(0, 255) as u8;
            }
            ColorMode::Hsl => {
                let mut hsl = rgb_to_hsl(self.rgb);
                match self.channel {
                    Channel::First => {
                        let value = hsl[0] as i32 + delta;
                        hsl[0] = value.clamp(0, 360) as u16;
                    }
                    Channel::Second => {
                        let value = hsl[1] as i32 + delta;
                        hsl[1] = value.clamp(0, 100) as u16;
                    }
                    Channel::Third => {
                        let value = hsl[2] as i32 + delta;
                        hsl[2] = value.clamp(0, 100) as u16;
                    }
                }
                self.rgb = hsl_to_rgb(hsl);
            }
        }
        self.reset_edit_buffer();
    }

    fn handle_digit(&mut self, ch: char) -> bool {
        self.ensure_edit_buffer();
        match self.mode {
            ColorMode::Hex => self.handle_hex_digit(ch),
            ColorMode::Rgb => self.handle_rgb_digit(ch),
            ColorMode::Hsl => self.handle_hsl_digit(ch),
        }
    }

    fn handle_hex_digit(&mut self, ch: char) -> bool {
        let Some(digit) = hex_value(ch) else {
            return false;
        };

        if self.edit_buffer.len() >= 2 {
            self.edit_buffer.clear();
        }
        self.edit_buffer.push(ch.to_ascii_uppercase());

        let value = if self.edit_buffer.len() == 1 {
            digit * 16
        } else {
            parse_hex_pair_prefix(self.edit_buffer.as_str()).unwrap_or(digit * 16)
        };

        self.rgb[self.channel.index()] = value;
        true
    }

    fn handle_rgb_digit(&mut self, ch: char) -> bool {
        if !ch.is_ascii_digit() {
            return false;
        }
        if self.edit_buffer.len() >= 3 {
            self.edit_buffer.clear();
        }
        self.edit_buffer.push(ch);
        let value = parse_clamped_u8(self.edit_buffer.as_str(), 255);
        self.rgb[self.channel.index()] = value;
        true
    }

    fn handle_hsl_digit(&mut self, ch: char) -> bool {
        if !ch.is_ascii_digit() {
            return false;
        }
        if self.edit_buffer.len() >= 3 {
            self.edit_buffer.clear();
        }
        self.edit_buffer.push(ch);
        self.set_hsl_channel_from_buffer();
        true
    }

    fn handle_backspace(&mut self) -> bool {
        if self.edit_buffer.is_empty() {
            return false;
        }

        self.edit_buffer.pop();
        if self.edit_buffer.is_empty() {
            return true;
        }

        match self.mode {
            ColorMode::Hex => {
                let value = parse_hex_high_nibble(self.edit_buffer.as_str()).unwrap_or(0);
                self.rgb[self.channel.index()] = value;
            }
            ColorMode::Rgb => {
                let value = parse_clamped_u8(self.edit_buffer.as_str(), 255);
                self.rgb[self.channel.index()] = value;
            }
            ColorMode::Hsl => self.set_hsl_channel_from_buffer(),
        }

        true
    }

    fn set_hsl_channel_from_buffer(&mut self) {
        let mut hsl = rgb_to_hsl(self.rgb);
        match self.channel {
            Channel::First => hsl[0] = parse_clamped_u16(self.edit_buffer.as_str(), 360),
            Channel::Second => hsl[1] = parse_clamped_u16(self.edit_buffer.as_str(), 100),
            Channel::Third => hsl[2] = parse_clamped_u16(self.edit_buffer.as_str(), 100),
        }
        self.rgb = hsl_to_rgb(hsl);
    }

    fn render_parts(&self, focused: bool) -> (Vec<Span>, usize) {
        let mut parts = Vec::<(String, Option<Channel>)>::new();
        parts.push(("■ ".to_string(), None));

        match self.mode {
            ColorMode::Hex => {
                let hex = rgb_to_hex(self.rgb);
                parts.push(("#".to_string(), None));
                parts.push((hex[1..3].to_string(), Some(Channel::First)));
                parts.push((hex[3..5].to_string(), Some(Channel::Second)));
                parts.push((hex[5..7].to_string(), Some(Channel::Third)));
            }
            ColorMode::Rgb => {
                parts.push(("R:".to_string(), None));
                parts.push((format!("{:>3}", self.rgb[0]), Some(Channel::First)));
                parts.push((" ".to_string(), None));
                parts.push(("G:".to_string(), None));
                parts.push((format!("{:>3}", self.rgb[1]), Some(Channel::Second)));
                parts.push((" ".to_string(), None));
                parts.push(("B:".to_string(), None));
                parts.push((format!("{:>3}", self.rgb[2]), Some(Channel::Third)));
            }
            ColorMode::Hsl => {
                let hsl = rgb_to_hsl(self.rgb);
                parts.push(("H:".to_string(), None));
                parts.push((format!("{:>3}", hsl[0]), Some(Channel::First)));
                parts.push((" ".to_string(), None));
                parts.push(("S:".to_string(), None));
                parts.push((format!("{:>3}", hsl[1]), Some(Channel::Second)));
                parts.push((" ".to_string(), None));
                parts.push(("L:".to_string(), None));
                parts.push((format!("{:>3}", hsl[2]), Some(Channel::Third)));
            }
        }

        let active_style = Style::new().color(Color::Cyan).bold();
        let inactive_style = Style::default();

        let mut spans = Vec::<Span>::new();
        let mut offset = 0usize;
        let mut cursor_offset = 0usize;
        for (idx, (text, channel)) in parts.into_iter().enumerate() {
            let mut style = inactive_style;
            if idx == 0 {
                style = Style::new().color(Color::Rgb(self.rgb[0], self.rgb[1], self.rgb[2]));
            }
            if focused && channel == Some(self.channel) {
                style = active_style;
                cursor_offset = offset;
            }
            spans.push(Span::styled(text.clone(), style).no_wrap());
            offset += UnicodeWidthStr::width(text.as_str());
        }

        (spans, cursor_offset)
    }
}

impl Drawable for ColorInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let (parts, _) = self.render_parts(focused);
        DrawOutput { lines: vec![parts] }
    }
}

impl Interactive for ColorInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(' ') => {
                self.cycle_mode();
                InteractionResult::handled()
            }
            KeyCode::Left => {
                self.channel = self.channel.prev();
                self.reset_edit_buffer();
                InteractionResult::handled()
            }
            KeyCode::Right => {
                self.channel = self.channel.next();
                self.reset_edit_buffer();
                InteractionResult::handled()
            }
            KeyCode::Up => {
                let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    10
                } else {
                    1
                };
                self.adjust_channel(step);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    10
                } else {
                    1
                };
                self.adjust_channel(-step);
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if self.handle_backspace() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Char(ch) => {
                if self.handle_digit(ch) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter => InteractionResult::submit_or_produce(
                self.submit_target.as_deref(),
                Value::Text(rgb_to_hex(self.rgb)),
            ),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(rgb_to_hex(self.rgb)))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.as_text()
            && let Some(rgb) = parse_hex(text)
        {
            self.rgb = rgb;
            self.reset_edit_buffer();
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, rgb_to_hex(self.rgb).as_str())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let (_, local) = self.render_parts(true);
        Some(CursorPos {
            col: local as u16,
            row: 0,
        })
    }
}

fn rgb_to_hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let raw = value.trim();
    let raw = raw.strip_prefix('#').unwrap_or(raw);
    if raw.len() != 6 || !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    let r = u8::from_str_radix(&raw[0..2], 16).ok()?;
    let g = u8::from_str_radix(&raw[2..4], 16).ok()?;
    let b = u8::from_str_radix(&raw[4..6], 16).ok()?;
    Some([r, g, b])
}

fn hex_value(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some(ch as u8 - b'0'),
        'a'..='f' => Some(10 + (ch as u8 - b'a')),
        'A'..='F' => Some(10 + (ch as u8 - b'A')),
        _ => None,
    }
}

fn parse_hex_pair_prefix(value: &str) -> Option<u8> {
    let mut chars = value.chars();
    let hi = chars.next().and_then(hex_value)?;
    let lo = chars.next().and_then(hex_value)?;
    Some(hi * 16 + lo)
}

fn parse_hex_high_nibble(value: &str) -> Option<u8> {
    value.chars().next().and_then(hex_value).map(|hi| hi * 16)
}

fn parse_clamped_u8(value: &str, max: i32) -> u8 {
    value.parse::<i32>().unwrap_or(0).clamp(0, max) as u8
}

fn parse_clamped_u16(value: &str, max: i32) -> u16 {
    value.parse::<i32>().unwrap_or(0).clamp(0, max) as u16
}

fn rgb_to_hsl(rgb: [u8; 3]) -> [u16; 3] {
    let r = rgb[0] as f64 / 255.0;
    let g = rgb[1] as f64 / 255.0;
    let b = rgb[2] as f64 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let mut hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    if hue < 0.0 {
        hue += 360.0;
    }

    let lightness = (max + min) / 2.0;
    let saturation = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * lightness - 1.0).abs())
    };

    [
        hue.round() as u16,
        (saturation * 100.0).round() as u16,
        (lightness * 100.0).round() as u16,
    ]
}

fn hsl_to_rgb(hsl: [u16; 3]) -> [u8; 3] {
    let h = (hsl[0] as f64 % 360.0) / 360.0;
    let s = (hsl[1] as f64 / 100.0).clamp(0.0, 1.0);
    let l = (hsl[2] as f64 / 100.0).clamp(0.0, 1.0);

    if s == 0.0 {
        let gray = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return [gray, gray, gray];
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    [
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    ]
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> u8 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }

    let value = if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    };

    (value * 255.0).round().clamp(0.0, 255.0) as u8
}
