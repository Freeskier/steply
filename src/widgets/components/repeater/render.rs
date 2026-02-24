use super::*;
use crate::widgets::shared::render_ctx::child_context_for;

impl Repeater {
    fn child_context(
        &self,
        ctx: &RenderContext,
        focused_child_id: Option<String>,
    ) -> RenderContext {
        child_context_for(self.base.id(), ctx, focused_child_id)
    }

    fn child_draw_line(
        &self,
        ctx: &RenderContext,
        row_idx: usize,
        field_idx: usize,
        focused: bool,
    ) -> SpanLine {
        let Some(row) = self.rows.get(row_idx) else {
            return vec![Span::new("").no_wrap()];
        };
        let Some(widget) = row.fields.get(field_idx) else {
            return vec![Span::new("").no_wrap()];
        };

        let focused_id = if focused {
            Some(widget.id().to_string())
        } else {
            None
        };
        let child_ctx = self.child_context(ctx, focused_id);
        widget
            .draw(&child_ctx)
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()])
    }

    pub(super) fn line_prefix_rows(&self) -> usize {
        let mut rows = 0usize;
        if self.show_label && !self.base.label().is_empty() {
            rows += 1;
        }
        rows += 1;
        if self.progress_line().is_some() {
            rows += 1;
        }
        rows
    }

    fn draw_single_field(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        let field_label = self.active_field_label();
        let marker_style = if focused {
            Style::new().color(Color::Cyan).bold()
        } else {
            Style::new().color(Color::DarkGrey)
        };
        let mut line = vec![
            Span::styled("❯ ", marker_style).no_wrap(),
            Span::styled(format!("{field_label}: "), Style::new().bold()).no_wrap(),
        ];
        line.extend(self.child_draw_line(ctx, self.active_item, self.active_field, focused));
        lines.push(line);
        lines
    }

    fn draw_stacked_fields(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        for (field_idx, field) in self.fields.iter().enumerate() {
            let is_active = field_idx == self.active_field;
            let marker = if is_active { "❯ " } else { "  " };
            let marker_style = if is_active && focused {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::new().color(Color::DarkGrey)
            };
            let label_style = if is_active && focused {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::new().bold()
            };
            let mut line = vec![
                Span::styled(marker, marker_style).no_wrap(),
                Span::styled(format!("{}: ", field.label), label_style).no_wrap(),
            ];
            line.extend(self.child_draw_line(
                ctx,
                self.active_item,
                field_idx,
                focused && is_active,
            ));
            lines.push(line);
        }
        lines
    }

    fn draw_empty_state(&self) -> Vec<SpanLine> {
        if self.rows.is_empty() {
            return vec![vec![
                Span::styled(
                    "No items to configure.",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]];
        }
        if self.fields.is_empty() {
            return vec![vec![
                Span::styled(
                    "No repeater fields configured.",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]];
        }
        vec![]
    }
}

impl Drawable for Repeater {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let mut lines = Vec::<SpanLine>::new();

        if self.show_label && !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        lines.push(vec![
            Span::styled(self.header_line(), Style::new().color(Color::Yellow).bold()).no_wrap(),
        ]);

        if let Some(progress) = self.progress_line() {
            lines.push(vec![
                Span::styled(progress, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        let mut body = self.draw_empty_state();
        if body.is_empty() {
            body = match self.layout {
                RepeaterLayout::SingleField => self.draw_single_field(ctx, focused),
                RepeaterLayout::Stacked => self.draw_stacked_fields(ctx, focused),
            };
        }
        lines.extend(body);

        decorate_component_validation(&mut lines, ctx, self.base.id());

        DrawOutput { lines }
    }
}
