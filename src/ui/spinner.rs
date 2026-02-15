use crate::ui::span::Span;
use crate::ui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerStyle {
    #[default]
    Braille,
    Dots,
    Arc,
    Line,
}

const BRAILLE: &[char] = &['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];
const DOTS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const ARC: &[char] = &['◜', '◠', '◝', '◞', '◡', '◟'];
const LINE: &[char] = &['|', '/', '—', '\\'];

#[derive(Debug, Clone)]
pub struct Spinner {
    frame: u8,
    style: SpinnerStyle,
}

impl Spinner {
    pub fn new(style: SpinnerStyle) -> Self {
        Self { frame: 0, style }
    }

    pub fn tick(&mut self) {
        let len = self.frames().len() as u8;
        self.frame = (self.frame + 1) % len;
    }

    pub fn glyph(&self) -> char {
        let frames = self.frames();
        frames[self.frame as usize % frames.len()]
    }

    pub fn span(&self) -> Span {
        Span::styled(self.glyph().to_string(), Style::new().color(Color::Cyan)).no_wrap()
    }

    fn frames(&self) -> &'static [char] {
        match self.style {
            SpinnerStyle::Braille => BRAILLE,
            SpinnerStyle::Dots => DOTS,
            SpinnerStyle::Arc => ARC,
            SpinnerStyle::Line => LINE,
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new(SpinnerStyle::default())
    }
}
