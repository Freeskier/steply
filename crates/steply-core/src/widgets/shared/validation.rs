use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::traits::RenderContext;

pub fn decorate_component_validation(lines: &mut Vec<SpanLine>, ctx: &RenderContext, id: &str) {
    if let Some(error) = ctx.visible_errors.get(id) {
        lines.push(vec![
            Span::styled(
                format!("✗ {}", error),
                Style::new().color(Color::Red).bold(),
            )
            .no_wrap(),
        ]);
        return;
    }

    if ctx.invalid_hidden.contains(id) {
        tint_unstyled(lines.as_mut_slice(), Color::Red);
    }
}

fn tint_unstyled(lines: &mut [SpanLine], color: Color) {
    for line in lines {
        for span in line {
            if span.style.color.is_none() {
                span.style.color = Some(color);
            }
        }
    }
}
