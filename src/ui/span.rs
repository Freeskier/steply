use crate::ui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub wrap_mode: WrapMode,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
            wrap_mode: WrapMode::Wrap,
        }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            wrap_mode: WrapMode::Wrap,
        }
    }

    pub fn no_wrap(mut self) -> Self {
        self.wrap_mode = WrapMode::NoWrap;
        self
    }
}

pub type SpanLine = Vec<Span>;
