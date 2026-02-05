use crate::core::node::Node;
use crate::core::step::Step;
use crate::inputs::Input;
use crate::ui::theme::Theme;
use crate::ui::{render::RenderLine, span::Span};
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

    pub fn render_node_lines(&self, node: &Node) -> Vec<RenderLine> {
        match node {
            Node::Component(component) => {
                let mut lines = component.render(self);
                if component.render_children() {
                    if let Some(children) = component.children() {
                        for child in children {
                            lines.extend(self.render_node_lines(child));
                        }
                    }
                }
                lines
            }
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                let (spans, cursor_offset) =
                    self.render_input_full(input.as_ref(), inline_error, input.is_focused());
                vec![RenderLine {
                    spans,
                    cursor_offset,
                }]
            }
            Node::Text(text) => vec![self.render_text_line(text)],
        }
    }

    pub fn render_separator(&self) -> RenderLine {
        RenderLine {
            spans: vec![Span::new("─".repeat(20)).with_style(self.theme.hint.clone())],
            cursor_offset: None,
        }
    }

    pub fn render_text_line(&self, text: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(text)],
            cursor_offset: None,
        }
    }

    pub fn render_prompt_line(&self, prompt: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(prompt).with_style(self.theme.prompt.clone())],
            cursor_offset: None,
        }
    }

    pub fn render_hint_line(&self, hint: &str) -> RenderLine {
        RenderLine {
            spans: vec![Span::new(hint).with_style(self.theme.hint.clone())],
            cursor_offset: None,
        }
    }

    pub fn render_input_full(
        &self,
        input: &dyn Input,
        inline_error: bool,
        focused: bool,
    ) -> (Vec<Span>, Option<usize>) {
        let mut spans = Vec::new();

        spans.push(Span::new(input.label()));
        spans.push(Span::new(": "));
        spans.extend(self.render_input_content(input, inline_error, focused));

        let cursor_offset = if focused {
            Some(self.calculate_cursor_offset(input, input.label().width() + 2))
        } else {
            None
        };

        (spans, cursor_offset)
    }

    pub fn render_input_field(
        &self,
        input: &dyn Input,
        inline_error: bool,
        focused: bool,
    ) -> (Vec<Span>, Option<usize>) {
        let spans = self.render_input_content(input, inline_error, true);
        let cursor_offset = if focused {
            Some(self.calculate_cursor_offset(input, 0))
        } else {
            None
        };
        (spans, cursor_offset)
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

pub struct StepRenderer<'a> {
    theme: &'a Theme,
}

impl<'a> StepRenderer<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    pub fn build(&self, step: &Step) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        let ctx = RenderContext::new(self.theme);

        let inline_input = self.find_inline_input(step);

        if let Some(line) = self.build_prompt(step, inline_input, &ctx) {
            lines.push(line);
        }

        if let Some(line) = self.build_hint(step, &ctx) {
            lines.push(line);
        }

        if inline_input.is_none() || step.prompt.is_empty() {
            lines.extend(self.build_nodes(step, &ctx));
        }

        lines
    }

    fn find_inline_input<'b>(&self, step: &'b Step) -> Option<&'b Node> {
        if step.nodes.len() != 1 {
            return None;
        }

        let node = step.nodes.first()?;
        if node.is_input() { Some(node) } else { None }
    }

    fn build_prompt(
        &self,
        step: &Step,
        inline_input: Option<&Node>,
        ctx: &RenderContext,
    ) -> Option<RenderLine> {
        if step.prompt.is_empty() {
            return None;
        }

        let prompt_style = ctx.theme().prompt.clone();

        if let Some(node) = inline_input {
            let mut spans = vec![
                Span::new(&step.prompt).with_style(prompt_style),
                Span::new(" "),
            ];

            let (input_spans, field_cursor) = self.render_node_field(node, ctx);
            spans.extend(input_spans);

            let prompt_width = step.prompt.width();
            let cursor_offset = field_cursor.map(|offset| offset + prompt_width + 1);

            Some(RenderLine {
                spans,
                cursor_offset,
            })
        } else {
            Some(ctx.render_prompt_line(&step.prompt))
        }
    }

    fn build_hint(&self, step: &Step, ctx: &RenderContext) -> Option<RenderLine> {
        let hint = step.hint.as_ref()?;
        if hint.is_empty() {
            return None;
        }

        Some(ctx.render_hint_line(hint))
    }

    fn build_nodes(&self, step: &Step, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        for node in &step.nodes {
            lines.extend(ctx.render_node_lines(node));
        }

        lines
    }

    pub fn render_node(&self, node: &Node) -> (Vec<Span>, Option<usize>) {
        let ctx = RenderContext::new(self.theme);
        self.render_node_full(node, &ctx)
    }

    pub fn render_node_lines(&self, node: &Node) -> Vec<RenderLine> {
        let ctx = RenderContext::new(self.theme);
        ctx.render_node_lines(node)
    }

    fn render_node_full(&self, node: &Node, ctx: &RenderContext) -> (Vec<Span>, Option<usize>) {
        match node {
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                ctx.render_input_full(input.as_ref(), inline_error, input.is_focused())
            }
            Node::Text(text) => (vec![Span::new(text)], None),
            Node::Component(component) => {
                let lines = component.render(ctx);
                let (spans, cursor_offset) = lines
                    .first()
                    .map(|line| (line.spans.clone(), line.cursor_offset))
                    .unwrap_or_default();
                (spans, cursor_offset)
            }
        }
    }

    fn render_node_field(&self, node: &Node, ctx: &RenderContext) -> (Vec<Span>, Option<usize>) {
        match node {
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                ctx.render_input_field(input.as_ref(), inline_error, input.is_focused())
            }
            _ => (vec![], None),
        }
    }
}
