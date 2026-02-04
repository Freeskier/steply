use crate::inputs::{Input, InputBase, InputCaps, KeyResult};
use crate::span::Span;
use crate::style::{Color, Style};
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;
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
            Channel::First => Channel::Second,
            Channel::Second => Channel::Third,
            Channel::Third => Channel::First,
        }
    }

    fn prev(self) -> Self {
        match self {
            Channel::First => Channel::Third,
            Channel::Second => Channel::First,
            Channel::Third => Channel::Second,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

pub struct ColorInput {
    base: InputBase,
    rgb: [u8; 3],
    mode: ColorMode,
    channel: Channel,
    edit_buffer: String,
    edit_mode: ColorMode,
    edit_channel: Channel,
}

impl ColorInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            rgb: [0, 0, 0],
            mode: ColorMode::Hex,
            channel: Channel::First,
            edit_buffer: String::new(),
            edit_mode: ColorMode::Hex,
            edit_channel: Channel::First,
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

    pub fn with_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.rgb = [r, g, b];
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
        self.base.error = None;
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
            let hi = hex_value(self.edit_buffer.chars().nth(0).unwrap()).unwrap();
            let lo = hex_value(self.edit_buffer.chars().nth(1).unwrap()).unwrap();
            (hi * 16 + lo) as u8
        };

        let idx = self.channel.index();
        self.rgb[idx] = value;
        self.base.error = None;
        true
    }

    fn handle_rgb_digit(&mut self, ch: char) -> bool {
        if !ch.is_ascii_digit() {
            return false;
        }
        let max_len = 3;
        if self.edit_buffer.len() >= max_len {
            self.edit_buffer.clear();
        }
        self.edit_buffer.push(ch);
        let value = self.edit_buffer.parse::<i32>().unwrap_or(0).clamp(0, 255) as u8;
        let idx = self.channel.index();
        self.rgb[idx] = value;
        self.base.error = None;
        true
    }

    fn handle_hsl_digit(&mut self, ch: char) -> bool {
        if !ch.is_ascii_digit() {
            return false;
        }

        let max_len = match self.channel {
            Channel::First => 3,
            Channel::Second | Channel::Third => 3,
        };
        if self.edit_buffer.len() >= max_len {
            self.edit_buffer.clear();
        }
        self.edit_buffer.push(ch);

        let mut hsl = rgb_to_hsl(self.rgb);
        let value = self.edit_buffer.parse::<i32>().unwrap_or(0);
        match self.channel {
            Channel::First => hsl[0] = value.clamp(0, 360) as u16,
            Channel::Second => hsl[1] = value.clamp(0, 100) as u16,
            Channel::Third => hsl[2] = value.clamp(0, 100) as u16,
        }
        self.rgb = hsl_to_rgb(hsl);
        self.base.error = None;
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
                let hi = hex_value(self.edit_buffer.chars().nth(0).unwrap()).unwrap();
                let value = hi * 16;
                let idx = self.channel.index();
                self.rgb[idx] = value;
            }
            ColorMode::Rgb => {
                let value = self.edit_buffer.parse::<i32>().unwrap_or(0).clamp(0, 255) as u8;
                let idx = self.channel.index();
                self.rgb[idx] = value;
            }
            ColorMode::Hsl => {
                let mut hsl = rgb_to_hsl(self.rgb);
                let value = self.edit_buffer.parse::<i32>().unwrap_or(0);
                match self.channel {
                    Channel::First => hsl[0] = value.clamp(0, 360) as u16,
                    Channel::Second => hsl[1] = value.clamp(0, 100) as u16,
                    Channel::Third => hsl[2] = value.clamp(0, 100) as u16,
                }
                self.rgb = hsl_to_rgb(hsl);
            }
        }
        self.base.error = None;
        true
    }

    fn render_with_cursor(&self, theme: &crate::theme::Theme) -> (Vec<Span>, usize) {
        let mut spans = Vec::new();
        let mut cursor_pos = 0usize;
        let mut offset = 0usize;

        let preview_style = preview_style(self.rgb);
        spans.push(Span::new("■").with_style(preview_style));
        spans.push(Span::new(" "));
        offset += 3;

        let parts: Vec<(String, Option<Channel>)> = match self.mode {
            ColorMode::Hex => {
                let hex = rgb_to_hex(self.rgb);
                let part_a = hex[1..3].to_string();
                let part_b = hex[3..5].to_string();
                let part_c = hex[5..7].to_string();
                vec![
                    ("#".to_string(), None),
                    (part_a, Some(Channel::First)),
                    (part_b, Some(Channel::Second)),
                    (part_c, Some(Channel::Third)),
                ]
            }
            ColorMode::Rgb => vec![
                ("R:".to_string(), None),
                (format!("{:>3}", self.rgb[0]), Some(Channel::First)),
                (" ".to_string(), None),
                ("G:".to_string(), None),
                (format!("{:>3}", self.rgb[1]), Some(Channel::Second)),
                (" ".to_string(), None),
                ("B:".to_string(), None),
                (format!("{:>3}", self.rgb[2]), Some(Channel::Third)),
            ],
            ColorMode::Hsl => {
                let hsl = rgb_to_hsl(self.rgb);
                vec![
                    ("H:".to_string(), None),
                    (format!("{:>3}", hsl[0]), Some(Channel::First)),
                    (" ".to_string(), None),
                    ("S:".to_string(), None),
                    (format!("{:>3}", hsl[1]), Some(Channel::Second)),
                    (" ".to_string(), None),
                    ("L:".to_string(), None),
                    (format!("{:>3}", hsl[2]), Some(Channel::Third)),
                ]
            }
        };

        for (text, channel) in parts {
            let mut span = Span::new(text.clone());
            if channel == Some(self.channel) && self.base.focused {
                span = span.with_style(theme.focused.clone());
                cursor_pos = offset;
            }
            spans.push(span);
            offset += text.width();
        }

        (spans, cursor_pos)
    }
}

impl Input for ColorInput {
    fn base(&self) -> &InputBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut InputBase {
        &mut self.base
    }

    fn value(&self) -> String {
        rgb_to_hex(self.rgb)
    }

    fn set_value(&mut self, value: String) {
        if let Some(rgb) = parse_hex(&value) {
            self.rgb = rgb;
        }
    }

    fn raw_value(&self) -> String {
        self.value()
    }

    fn is_complete(&self) -> bool {
        true
    }

    fn cursor_pos(&self) -> usize {
        self.channel.index()
    }

    fn capabilities(&self) -> InputCaps {
        InputCaps {
            capture_ctrl_left: true,
            capture_ctrl_right: true,
            ..InputCaps::default()
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Char(' ') => {
                self.cycle_mode();
                KeyResult::Handled
            }
            KeyCode::Left => {
                self.channel = self.channel.prev();
                self.reset_edit_buffer();
                KeyResult::Handled
            }
            KeyCode::Right => {
                self.channel = self.channel.next();
                self.reset_edit_buffer();
                KeyResult::Handled
            }
            KeyCode::Up => {
                let step = if modifiers.contains(KeyModifiers::SHIFT) {
                    10
                } else {
                    1
                };
                self.adjust_channel(step);
                KeyResult::Handled
            }
            KeyCode::Down => {
                let step = if modifiers.contains(KeyModifiers::SHIFT) {
                    10
                } else {
                    1
                };
                self.adjust_channel(-step);
                KeyResult::Handled
            }
            KeyCode::Backspace => {
                if self.handle_backspace() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            KeyCode::Char(ch) => {
                if self.handle_digit(ch) {
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
        let (spans, _) = self.render_with_cursor(theme);
        spans
    }

    fn cursor_offset_in_content(&self) -> usize {
        let (_, offset) = self.render_with_cursor(&crate::theme::Theme::default());
        offset
    }
}

fn rgb_to_hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let text = value.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&text[0..2], 16).ok()?;
    let g = u8::from_str_radix(&text[2..4], 16).ok()?;
    let b = u8::from_str_radix(&text[4..6], 16).ok()?;
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

fn rgb_to_hsl(rgb: [u8; 3]) -> [u16; 3] {
    let r = rgb[0] as f64 / 255.0;
    let g = rgb[1] as f64 / 255.0;
    let b = rgb[2] as f64 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let mut h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    if h < 0.0 {
        h += 360.0;
    }

    let l = (max + min) / 2.0;
    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };

    [
        h.round() as u16,
        (s * 100.0).round() as u16,
        (l * 100.0).round() as u16,
    ]
}

fn hsl_to_rgb(hsl: [u16; 3]) -> [u8; 3] {
    let h = (hsl[0] as f64 % 360.0) / 360.0;
    let s = (hsl[1] as f64 / 100.0).clamp(0.0, 1.0);
    let l = (hsl[2] as f64 / 100.0).clamp(0.0, 1.0);

    if s == 0.0 {
        let gray = (l * 255.0).round() as u8;
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

fn preview_style(rgb: [u8; 3]) -> Style {
    Style::new().with_color(Color::Rgb(rgb[0], rgb[1], rgb[2]))
}
