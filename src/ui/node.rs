use crate::inputs::Input;
use crate::span::Span;
use crate::theme::Theme;
use unicode_width::UnicodeWidthStr;

pub enum Node {
    Text(String),
    Input(Box<dyn Input>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Full,
    Field,
}

impl Node {
    pub fn text(text: impl Into<String>) -> Self {
        Node::Text(text.into())
    }

    pub fn input(input: impl Input + 'static) -> Self {
        Node::Input(Box::new(input))
    }

    pub fn as_input(&self) -> Option<&dyn Input> {
        match self {
            Node::Input(input) => Some(input.as_ref()),
            _ => None,
        }
    }

    pub fn as_input_mut(&mut self) -> Option<&mut dyn Input> {
        match self {
            Node::Input(input) => Some(input.as_mut()),
            _ => None,
        }
    }

    pub fn render(&self, mode: RenderMode, inline_error_message: bool, theme: &Theme) -> Vec<Span> {
        match self {
            Node::Text(text) => vec![Span::new(text.clone())],
            Node::Input(input) => {
                let (show_label, always_brackets) = match mode {
                    RenderMode::Full => (true, false),
                    RenderMode::Field => (false, true),
                };
                Self::render_input(
                    input.as_ref(),
                    inline_error_message,
                    theme,
                    show_label,
                    always_brackets,
                )
            }
        }
    }

    fn render_input(
        input: &dyn Input,
        inline_error_message: bool,
        theme: &Theme,
        show_label: bool,
        always_brackets: bool,
    ) -> Vec<Span> {
        let mut spans = Vec::new();
        if show_label {
            spans.push(Span::new(input.label()));
            spans.push(Span::new(": "));
        }

        let content_spans = Self::content_spans(input, inline_error_message, theme);
        let content_width: usize = content_spans.iter().map(|s| s.text().width()).sum();
        let use_brackets = input.render_brackets() && (always_brackets || input.is_focused());

        if use_brackets {
            spans.push(Span::new("["));
        }

        spans.extend(content_spans);

        if use_brackets && content_width < input.min_width() {
            let padding = input.min_width() - content_width;
            spans.push(Span::new(" ".repeat(padding)));
        }

        if use_brackets {
            spans.push(Span::new("]"));
        }

        spans
    }

    fn content_spans(input: &dyn Input, inline_error_message: bool, theme: &Theme) -> Vec<Span> {
        let error_style = theme.error.clone();

        if inline_error_message {
            if let Some(err) = input.error() {
                return vec![
                    Span::new("✗ ").with_style(error_style.clone()),
                    Span::new(err).with_style(error_style.clone()),
                ];
            }
        }

        let mut spans = input.render_content(theme);
        if input.error().is_none() && input.value().is_empty() {
            let is_empty = spans.iter().all(|span| span.text().is_empty());
            if is_empty {
                if let Some(placeholder) = input.placeholder() {
                    spans = vec![Span::new(placeholder).with_style(theme.placeholder.clone())];
                }
            }
        }
        if input.error().is_some() {
            spans = spans
                .into_iter()
                .map(|span| {
                    let merged = span.style().clone().merge(&error_style);
                    span.with_style(merged)
                })
                .collect();
        }
        spans
    }

    pub fn cursor_offset(&self) -> Option<usize> {
        match self {
            Node::Input(input) if input.is_focused() => {
                let label_len = input.label().width() + 2;
                let bracket_len = if input.render_brackets() { 1 } else { 0 };
                let content_offset = input.cursor_offset_in_content();
                Some(label_len + bracket_len + content_offset)
            }
            _ => None,
        }
    }

    pub fn cursor_offset_in_field(&self) -> Option<usize> {
        match self {
            Node::Input(input) if input.is_focused() => {
                let bracket_len = if input.render_brackets() { 1 } else { 0 };
                let content_offset = input.cursor_offset_in_content();
                Some(bracket_len + content_offset)
            }
            _ => None,
        }
    }
}
