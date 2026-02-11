use crate::style::Style;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    Yes,
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    text: String,
    style: Style,
    wrap: Wrap,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
            wrap: Wrap::Yes,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn wrap(&self) -> Wrap {
        self.wrap
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn width(&self) -> usize {
        self.text.width()
    }

    pub fn split_at_width(&self, max: usize) -> (Span, Option<Span>) {
        if max == 0 {
            return (self.clone_empty(), Some(self.clone()));
        }

        let total_width = self.width();
        if total_width <= max {
            return (self.clone(), None);
        }

        let mut current_width = 0;
        let mut split_idx = 0;
        for (idx, ch) in self.text.char_indices() {
            let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + char_width > max {
                break;
            }
            current_width += char_width;
            split_idx = idx + ch.len_utf8();
        }

        let (left, right) = self.text.split_at(split_idx);

        let tail = if right.is_empty() {
            None
        } else {
            Some(self.clone_with_text(right))
        };

        (self.clone_with_text(left), tail)
    }

    fn clone_empty(&self) -> Span {
        Span {
            text: String::new(),
            style: self.style.clone(),
            wrap: self.wrap,
        }
    }

    fn clone_with_text(&self, text: &str) -> Span {
        Span {
            text: text.to_string(),
            style: self.style.clone(),
            wrap: self.wrap,
        }
    }
}
