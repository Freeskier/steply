use crate::core::component::ComponentItem;
use crate::core::node::Node;
use crate::core::node_registry::NodeRegistry;
use crate::core::step::Step;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::ui::theme::Theme;
use unicode_width::UnicodeWidthStr;

pub struct RenderLine {
    pub spans: Vec<Span>,
    pub cursor_offset: Option<usize>,
}

pub struct StepRenderer<'a> {
    theme: &'a Theme,
}

impl<'a> StepRenderer<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    pub fn build(&self, step: &Step, registry: &NodeRegistry) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        let inline_input = self.find_inline_input(step, registry);

        if let Some(line) = self.build_prompt(step, registry, inline_input) {
            lines.push(line);
        }

        if let Some(line) = self.build_hint(step) {
            lines.push(line);
        }

        if inline_input.is_none() || step.prompt.is_empty() {
            lines.extend(self.build_nodes(step, registry));
        }

        lines
    }

    fn find_inline_input<'b>(&self, step: &Step, registry: &'b NodeRegistry) -> Option<&'b Node> {
        let input_count = step
            .node_ids
            .iter()
            .filter(|id| registry.get(id).map(|n| n.is_input()).unwrap_or(false))
            .count();

        if input_count != 1 || step.node_ids.len() != 1 {
            return None;
        }

        let id = step.node_ids.first()?;
        let node = registry.get(id)?;
        if node.is_input() { Some(node) } else { None }
    }

    fn build_prompt(
        &self,
        step: &Step,
        _registry: &NodeRegistry,
        inline_input: Option<&Node>,
    ) -> Option<RenderLine> {
        if step.prompt.is_empty() {
            return None;
        }

        let prompt_style = self.theme.prompt.clone();

        if let Some(node) = inline_input {
            let mut spans = vec![
                Span::new(&step.prompt).with_style(prompt_style),
                Span::new(" "),
            ];

            let (input_spans, field_cursor) = self.render_node_field(node);
            spans.extend(input_spans);

            let prompt_width = step.prompt.width();
            let cursor_offset = field_cursor.map(|offset| offset + prompt_width + 1);

            Some(RenderLine {
                spans,
                cursor_offset,
            })
        } else {
            Some(RenderLine {
                spans: vec![Span::new(&step.prompt).with_style(prompt_style)],
                cursor_offset: None,
            })
        }
    }

    fn build_hint(&self, step: &Step) -> Option<RenderLine> {
        let hint = step.hint.as_ref()?;
        if hint.is_empty() {
            return None;
        }

        Some(RenderLine {
            spans: vec![Span::new(hint).with_style(self.theme.hint.clone())],
            cursor_offset: None,
        })
    }

    fn build_nodes(&self, step: &Step, registry: &NodeRegistry) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        for id in &step.node_ids {
            let Some(node) = registry.get(id) else {
                continue;
            };

            lines.extend(self.render_node_lines(node, registry));
        }

        lines
    }

    pub fn render_node(&self, node: &Node) -> (Vec<Span>, Option<usize>) {
        self.render_node_full(node)
    }

    pub fn render_node_lines(&self, node: &Node, registry: &NodeRegistry) -> Vec<RenderLine> {
        match node {
            Node::Component(component) => self.render_component_lines(component.as_ref(), registry),
            _ => {
                let (spans, cursor_offset) = self.render_node_full(node);
                vec![RenderLine {
                    spans,
                    cursor_offset,
                }]
            }
        }
    }

    fn render_node_full(&self, node: &Node) -> (Vec<Span>, Option<usize>) {
        match node {
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                let spans = self.render_input_full(input.as_ref(), inline_error);
                let cursor_offset = if input.is_focused() {
                    Some(self.calculate_cursor_offset(input.as_ref(), input.label().width() + 2))
                } else {
                    None
                };
                (spans, cursor_offset)
            }
            Node::Text(text) => (vec![Span::new(text)], None),
            Node::Separator => (
                vec![Span::new("─".repeat(20)).with_style(self.theme.hint.clone())],
                None,
            ),
            Node::Component(_) => (vec![], None),
        }
    }

    fn render_node_field(&self, node: &Node) -> (Vec<Span>, Option<usize>) {
        match node {
            Node::Input(input) => {
                let inline_error = input.has_visible_error();
                let spans = self.render_input_field(input.as_ref(), inline_error);
                let cursor_offset = if input.is_focused() {
                    Some(self.calculate_cursor_offset(input.as_ref(), 0))
                } else {
                    None
                };
                (spans, cursor_offset)
            }
            _ => (vec![], None),
        }
    }

    fn render_component_lines(
        &self,
        component: &dyn crate::core::component::Component,
        registry: &NodeRegistry,
    ) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        for item in component.items(registry) {
            match item {
                ComponentItem::Node(id) => {
                    if let Some(node) = registry.get(&id) {
                        let (spans, cursor_offset) = self.render_node_full(node);
                        lines.push(RenderLine {
                            spans,
                            cursor_offset,
                        });
                    }
                }
                ComponentItem::Text(text) => {
                    lines.push(RenderLine {
                        spans: vec![Span::new(text)],
                        cursor_offset: None,
                    });
                }
                ComponentItem::Separator => {
                    lines.push(RenderLine {
                        spans: vec![Span::new("─".repeat(20)).with_style(self.theme.hint.clone())],
                        cursor_offset: None,
                    });
                }
                ComponentItem::Option {
                    cursor,
                    marker_left,
                    marker,
                    marker_right,
                    text,
                    active,
                    selected,
                } => {
                    let inactive_style = self.theme.hint.clone();
                    let marker_style = Style::new().with_color(Color::Green);
                    let cursor_style = Style::new().with_color(Color::Yellow);

                    let mut spans = Vec::new();
                    if active {
                        spans.push(Span::new(cursor).with_style(cursor_style));
                        spans.push(Span::new(" "));
                        spans.push(Span::new(marker_left));
                        if selected {
                            spans.push(Span::new(marker).with_style(marker_style));
                        } else {
                            spans.push(Span::new(marker));
                        }
                        spans.push(Span::new(marker_right));
                        spans.push(Span::new(" "));
                        spans.push(Span::new(text));
                    } else {
                        spans.push(Span::new(cursor).with_style(inactive_style.clone()));
                        spans.push(Span::new(" ").with_style(inactive_style.clone()));
                        spans.push(Span::new(marker_left).with_style(inactive_style.clone()));
                        if selected {
                            spans.push(Span::new(marker).with_style(marker_style));
                        } else {
                            spans.push(Span::new(marker).with_style(inactive_style.clone()));
                        }
                        spans.push(Span::new(marker_right).with_style(inactive_style.clone()));
                        spans.push(Span::new(" ").with_style(inactive_style.clone()));
                        spans.push(Span::new(text).with_style(inactive_style));
                    }

                    lines.push(RenderLine {
                        spans,
                        cursor_offset: None,
                    });
                }
            }
        }

        lines
    }

    fn render_input_full(&self, input: &dyn crate::inputs::Input, inline_error: bool) -> Vec<Span> {
        let mut spans = Vec::new();

        spans.push(Span::new(input.label()));
        spans.push(Span::new(": "));
        spans.extend(self.render_input_content(input, inline_error, input.is_focused()));

        spans
    }

    fn render_input_field(
        &self,
        input: &dyn crate::inputs::Input,
        inline_error: bool,
    ) -> Vec<Span> {
        self.render_input_content(input, inline_error, true)
    }

    fn render_input_content(
        &self,
        input: &dyn crate::inputs::Input,
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

    fn content_spans(&self, input: &dyn crate::inputs::Input, inline_error: bool) -> Vec<Span> {
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

    fn calculate_cursor_offset(&self, input: &dyn crate::inputs::Input, label_len: usize) -> usize {
        let bracket_len = if input.render_brackets() { 1 } else { 0 };
        label_len + bracket_len + input.cursor_offset_in_content()
    }
}
