use crate::core::node::Node;
use crate::inputs::Input;
use crate::ui::render::{RenderLine, RenderOutput};
use crate::ui::span::Span;
use crate::ui::theme::Theme;
use unicode_width::UnicodeWidthStr;

pub struct RenderContext<'a> {
    theme: &'a Theme,
}

impl<'a> RenderContext<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    pub fn theme(&self) -> &Theme {
        self.theme
    }

    pub fn render_node_lines(&self, node: &Node) -> RenderOutput {
        match node {
            Node::Component(component) => {
                let mut output = component.render(self);
                if component.render_children() {
                    if let Some(children) = component.children() {
                        for child in children {
                            output.append(self.render_node_lines(child));
                        }
                    }
                }
                output
            }
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                self.render_input_full(input.as_ref(), inline_error, input.is_focused())
            }
            Node::Text(text) => RenderOutput::from_line(self.render_text_line(text)),
        }
    }

    pub fn render_separator(&self) -> RenderLine {
        RenderLine {
            spans: vec![Span::new("─".repeat(20)).with_style(self.theme.hint.clone())],
        }
    }

    pub fn render_text_line(&self, text: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(text)],
        }
    }

    pub fn render_prompt_line(&self, prompt: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(prompt).with_style(self.theme.prompt.clone())],
        }
    }

    pub fn render_hint_line(&self, hint: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(hint).with_style(self.theme.hint.clone())],
        }
    }

    pub fn render_input_full(
        &self,
        input: &dyn Input,
        inline_error: bool,
        focused: bool,
    ) -> RenderOutput {
        let mut spans = Vec::new();

        spans.push(Span::new(input.label()));
        spans.push(Span::new(": "));
        spans.extend(self.render_input_content(input, inline_error, focused));

        let cursor_offset = if focused {
            Some(self.calculate_cursor_offset(input, input.label().width() + 2))
        } else {
            None
        };

        let mut output = RenderOutput::from_line(RenderLine { spans });
        if let Some(offset) = cursor_offset {
            output = output.with_cursor(0, offset);
        }
        output
    }

    pub fn render_input_field(
        &self,
        input: &dyn Input,
        inline_error: bool,
        focused: bool,
    ) -> RenderOutput {
        let spans = self.render_input_content(input, inline_error, true);
        let mut output = RenderOutput::from_line(RenderLine { spans });
        if focused {
            output = output.with_cursor(0, self.calculate_cursor_offset(input, 0));
        }
        output
    }

    fn render_input_content(
        &self,
        input: &dyn Input,
        inline_error: bool,
        with_brackets: bool,
    ) -> Vec<Span> {
        let mut spans = Vec::new();
        let use_brackets = input.render_brackets() && with_brackets;

        if use_brackets {
            spans.push(Span::new("["));
        }

        let content = self.content_spans(input, inline_error);
        let content_width: usize = content.iter().map(|s| s.width()).sum();
        spans.extend(content);

        if use_brackets && content_width < input.min_width() {
            let padding = input.min_width() - content_width;
            spans.push(Span::new(" ".repeat(padding)));
        }

        if use_brackets {
            spans.push(Span::new("]"));
        }

        spans
    }

    fn content_spans(&self, input: &dyn Input, inline_error: bool) -> Vec<Span> {
        let error_style = self.theme.error.clone();

        if inline_error {
            if let Some(err) = input.error().filter(|e| e.is_visible()) {
                return vec![
                    Span::new("✗ ").with_style(error_style.clone()),
                    Span::new(&err.message).with_style(error_style),
                ];
            }
        }

        let mut spans = input.render_content(self.theme);

        if !input.has_visible_error() && input.value().is_empty() {
            let is_empty = spans.iter().all(|s| s.text().is_empty());
            if is_empty {
                if let Some(placeholder) = input.placeholder() {
                    return vec![Span::new(placeholder).with_style(self.theme.placeholder.clone())];
                }
            }
        }

        if input.has_visible_error() {
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

    fn calculate_cursor_offset(&self, input: &dyn Input, label_len: usize) -> usize {
        let bracket_len = if input.render_brackets() { 1 } else { 0 };
        label_len + bracket_len + input.cursor_offset_in_content()
    }
}
