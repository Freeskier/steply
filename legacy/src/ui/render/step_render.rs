use crate::core::node::Node;
use crate::core::step::Step;
use crate::inputs::Input;
use crate::ui::render::{Render, RenderLine, RenderOutput};
use crate::ui::span::Span;
use unicode_width::UnicodeWidthStr;

impl Render for Step {
    fn render(&self, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
        let mut output = RenderOutput::empty();
        let inline_input = find_inline_input(self);

        if let Some(prompt_output) = build_prompt(self, inline_input, ctx) {
            output.append(prompt_output);
        }

        if let Some(hint_output) = build_hint(self, ctx) {
            output.append(hint_output);
        }

        if inline_input.is_none() || self.prompt.is_empty() {
            output.append(build_nodes(self, ctx));
        }

        output
    }
}

impl Render for Node {
    fn render(&self, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
        ctx.render_node_lines(self)
    }
}

impl Render for dyn Input {
    fn render(&self, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
        let inline_error = self.has_visible_error();
        ctx.render_input_full(self, inline_error, self.is_focused())
    }
}

impl Render for dyn crate::core::component::Component {
    fn render(&self, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
        crate::core::component::Component::render(self, ctx)
    }
}

fn find_inline_input<'b>(step: &'b Step) -> Option<&'b Node> {
    if step.nodes.len() != 1 {
        return None;
    }

    let node = step.nodes.first()?;
    if node.is_input() { Some(node) } else { None }
}

fn build_prompt(
    step: &Step,
    inline_input: Option<&Node>,
    ctx: &crate::ui::render::RenderContext<'_>,
) -> Option<RenderOutput> {
    if step.prompt.is_empty() {
        return None;
    }

    let prompt_style = ctx.theme().prompt.clone();

    if let Some(node) = inline_input {
        let mut spans = vec![
            Span::new(&step.prompt).with_style(prompt_style),
            Span::new(" "),
        ];

        let field_output = render_node_field(node, ctx);
        spans.extend(
            field_output
                .lines
                .first()
                .map(|line| line.spans.clone())
                .unwrap_or_default(),
        );

        let prompt_width = step.prompt.width();
        if let Some(cursor) = field_output.cursor {
            let output = RenderOutput::from_line(RenderLine { spans })
                .with_cursor(0, cursor.offset + prompt_width + 1);
            return Some(output);
        }

        Some(RenderOutput::from_line(RenderLine { spans }))
    } else {
        Some(RenderOutput::from_line(
            ctx.render_prompt_line(&step.prompt),
        ))
    }
}

fn build_hint(step: &Step, ctx: &crate::ui::render::RenderContext<'_>) -> Option<RenderOutput> {
    let hint = step.hint.as_ref()?;
    if hint.is_empty() {
        return None;
    }

    Some(RenderOutput::from_line(ctx.render_hint_line(hint)))
}

fn build_nodes(step: &Step, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
    let mut output = RenderOutput::empty();
    for node in &step.nodes {
        output.append(ctx.render_node_lines(node));
    }
    output
}

fn render_node_field(node: &Node, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput {
    match node {
        Node::Input(input) => {
            let inline_error = input.has_visible_error();
            ctx.render_input_field(input.as_ref(), inline_error, input.is_focused())
        }
        _ => RenderOutput::empty(),
    }
}
