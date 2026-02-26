use super::*;
use crate::widgets::shared::render_ctx::child_context_for;

impl Table {
    fn row_digits(&self) -> usize {
        self.rows.len().max(1).to_string().len()
    }

    fn row_index_width(&self) -> usize {
        if self.show_row_numbers {
            self.row_digits().saturating_add(2)
        } else {
            1
        }
    }

    fn row_index_line(&self, row_idx: usize) -> SpanLine {
        let active = self.focus == TableFocus::Body && self.active_row == row_idx;
        let marker = if active { '❯' } else { ' ' };
        let marker_style = if active {
            Style::new().color(Color::Yellow).bold()
        } else {
            Style::default()
        };
        if !self.show_row_numbers {
            return vec![Span::styled(marker.to_string(), marker_style).no_wrap()];
        }
        let number = format!("{:>w$}", row_idx + 1, w = self.row_digits());
        vec![
            Span::styled(marker.to_string(), marker_style).no_wrap(),
            Span::new(" ").no_wrap(),
            Span::new(number).no_wrap(),
        ]
    }

    fn row_marker_prefix(&self, row_idx: usize) -> SpanLine {
        let active = self.focus == TableFocus::Body && self.active_row == row_idx;
        let marker = if active { '❯' } else { ' ' };
        let marker_style = if active {
            Style::new().color(Color::Yellow).bold()
        } else {
            Style::default()
        };
        vec![
            Span::styled(marker.to_string(), marker_style).no_wrap(),
            Span::new(" ").no_wrap(),
        ]
    }

    fn sort_marker(&self, col_idx: usize) -> char {
        match self.sort {
            Some((idx, SortDirection::Asc)) if idx == col_idx => '↑',
            Some((idx, SortDirection::Desc)) if idx == col_idx => '↓',
            _ => '↕',
        }
    }

    fn header_text(&self, col_idx: usize) -> String {
        format!(
            "{} {}",
            self.columns[col_idx].header,
            self.sort_marker(col_idx)
        )
    }

    fn child_context(&self, ctx: &RenderContext, focused_cell_id: Option<String>) -> RenderContext {
        child_context_for(self.base.id(), ctx, focused_cell_id)
    }

    pub(super) fn fallback_context(&self) -> RenderContext {
        RenderContext::empty(TerminalSize {
            width: 80,
            height: 24,
        })
    }

    fn render_cell_line(
        &self,
        row_idx: usize,
        col_idx: usize,
        ctx: &RenderContext,
        focused: bool,
    ) -> SpanLine {
        let Some(row) = self.rows.get(row_idx) else {
            return vec![Span::new("").no_wrap()];
        };
        let Some(cell) = row.cells.get(col_idx) else {
            return vec![Span::new("").no_wrap()];
        };

        let focused_id = if focused {
            Some(cell.id().to_string())
        } else {
            None
        };
        let cell_ctx = self.child_context(ctx, focused_id);
        let mut line = cell
            .draw(&cell_ctx)
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()]);

        if focused {
            accent_active_cell(line.as_mut_slice());
        }

        let query = self.filter_query();
        let query = query.trim();
        if !query.is_empty() {
            let text = span_line_text(line.as_slice());
            if let Some((_, ranges)) = match_text(query, text.as_str()) {
                highlight_span_line(
                    &mut line,
                    ranges.as_slice(),
                    Style::new().color(Color::Yellow).bold(),
                );
            }
        }

        if !self.show_row_numbers && col_idx == 0 {
            let mut prefixed = self.row_marker_prefix(row_idx);
            prefixed.extend(line);
            line = prefixed;
        }
        line
    }

    pub(super) fn compute_column_widths(&self, ctx: &RenderContext) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .map(|(col_idx, col)| {
                let mut width = col
                    .min_width
                    .max(UnicodeWidthStr::width(self.header_text(col_idx).as_str()));
                for row_idx in self.visible_rows.iter().copied() {
                    let focused = self.edit_mode
                        && self.focus == TableFocus::Body
                        && self.active_row == row_idx
                        && self.active_col == col_idx;
                    let line = self.render_cell_line(row_idx, col_idx, ctx, focused);
                    width = width.max(Layout::line_width(line.as_slice()));
                }
                width
            })
            .collect()
    }

    fn render_grid(
        &self,
        ctx: &RenderContext,
        col_widths: &[usize],
        focused: bool,
    ) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }
        if self.filter.is_visible() {
            lines.push(filter_utils::render_filter_line_with(
                &self.filter,
                ctx,
                focused,
                |ctx, focused_id| self.child_context(ctx, focused_id),
            ));
        }

        let mut widths = Vec::<usize>::new();
        if self.show_row_numbers {
            widths.push(self.row_index_width());
        }
        widths.extend_from_slice(col_widths);

        lines.push(grid_border_line('┌', '┬', '┐', widths.as_slice()));

        let mut header_cells = Vec::<SpanLine>::with_capacity(widths.len());
        if self.show_row_numbers {
            header_cells.push(vec![Span::new("#").no_wrap()]);
        }
        for (col_idx, _) in self.columns.iter().enumerate() {
            let focused = self.focus == TableFocus::Header && self.active_col == col_idx;
            let sorted = self.sort.map(|(idx, _)| idx == col_idx).unwrap_or(false);
            let style = self.header_style(focused, sorted);
            let header_text = if !self.show_row_numbers && col_idx == 0 {
                format!("  {}", self.header_text(col_idx))
            } else {
                self.header_text(col_idx)
            };
            header_cells.push(vec![Span::styled(header_text, style).no_wrap()]);
        }
        lines.push(grid_row(header_cells, widths.as_slice()));
        lines.push(grid_border_line('├', '┼', '┤', widths.as_slice()));

        for row_idx in self.visible_rows.iter().copied() {
            let mut row_cells = Vec::<SpanLine>::with_capacity(widths.len());
            if self.show_row_numbers {
                row_cells.push(self.row_index_line(row_idx));
            }
            for (col_idx, _) in self.columns.iter().enumerate() {
                let focused = self.edit_mode
                    && self.focus == TableFocus::Body
                    && self.active_row == row_idx
                    && self.active_col == col_idx;
                row_cells.push(self.render_cell_line(row_idx, col_idx, ctx, focused));
            }
            lines.push(grid_row(row_cells, widths.as_slice()));
        }
        if self.rows.is_empty() {
            lines.push(grid_empty_row(widths.as_slice(), "(empty)"));
        }

        lines.push(grid_border_line('└', '┴', '┘', widths.as_slice()));
        lines
    }

    fn render_clean(
        &self,
        ctx: &RenderContext,
        col_widths: &[usize],
        focused: bool,
    ) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }
        if self.filter.is_visible() {
            lines.push(filter_utils::render_filter_line_with(
                &self.filter,
                ctx,
                focused,
                |ctx, focused_id| self.child_context(ctx, focused_id),
            ));
        }

        let mut header_cells = Vec::<SpanLine>::new();
        let mut clean_widths = Vec::<usize>::new();
        if self.show_row_numbers {
            header_cells.push(vec![Span::new("#").no_wrap()]);
            clean_widths.push(self.row_index_width());
        }
        clean_widths.extend_from_slice(col_widths);

        for (col_idx, _) in self.columns.iter().enumerate() {
            let focused = self.focus == TableFocus::Header && self.active_col == col_idx;
            let sorted = self.sort.map(|(idx, _)| idx == col_idx).unwrap_or(false);
            let style = self.header_style(focused, sorted);
            let header_text = if !self.show_row_numbers && col_idx == 0 {
                format!("  {}", self.header_text(col_idx))
            } else {
                self.header_text(col_idx)
            };
            header_cells.push(vec![Span::styled(header_text, style).no_wrap()]);
        }
        lines.push(clean_row(header_cells, clean_widths.as_slice()));

        for row_idx in self.visible_rows.iter().copied() {
            let mut row_cells = Vec::<SpanLine>::new();
            if self.show_row_numbers {
                row_cells.push(self.row_index_line(row_idx));
            }
            for (col_idx, _) in self.columns.iter().enumerate() {
                let focused = self.edit_mode
                    && self.focus == TableFocus::Body
                    && self.active_row == row_idx
                    && self.active_col == col_idx;
                row_cells.push(self.render_cell_line(row_idx, col_idx, ctx, focused));
            }
            lines.push(clean_row(row_cells, clean_widths.as_slice()));
        }
        if self.rows.is_empty() {
            lines.push(clean_empty_row(clean_widths.as_slice(), "(empty)"));
        }
        lines
    }

    pub(super) fn body_row_start(&self) -> u16 {
        let label_rows = if self.base.label().is_empty() { 0 } else { 1 };
        let filter_rows = if self.filter.is_visible() { 1 } else { 0 };
        match self.style {
            TableStyle::Grid => label_rows + filter_rows + 3,
            TableStyle::Clean => label_rows + filter_rows + 1,
        }
    }

    pub(super) fn body_col_starts(&self, col_widths: &[usize]) -> Vec<u16> {
        match self.style {
            TableStyle::Grid => {
                let mut widths = Vec::<usize>::new();
                if self.show_row_numbers {
                    widths.push(self.row_index_width());
                }
                widths.extend_from_slice(col_widths);

                let mut starts = Vec::<u16>::with_capacity(widths.len());
                let mut cursor = 2u16;
                for width in &widths {
                    starts.push(cursor);
                    cursor = cursor.saturating_add((*width as u16).saturating_add(3));
                }
                if self.show_row_numbers {
                    starts.into_iter().skip(1).collect()
                } else {
                    starts
                }
            }
            TableStyle::Clean => {
                let mut starts = Vec::<u16>::with_capacity(col_widths.len());
                let mut cursor = if self.show_row_numbers {
                    (self.row_index_width() as u16).saturating_add(2)
                } else {
                    0
                };
                for width in col_widths {
                    starts.push(cursor);
                    cursor = cursor.saturating_add((*width as u16).saturating_add(2));
                }
                starts
            }
        }
    }

    fn header_style(&self, focused: bool, sorted: bool) -> Style {
        if focused {
            Style::new().color(Color::Cyan).bold()
        } else if sorted {
            Style::new().color(Color::Green).bold()
        } else {
            Style::new().color(Color::White).bold()
        }
    }
}

impl Drawable for Table {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let col_widths = self.compute_column_widths(ctx);
        let mut lines = match self.style {
            TableStyle::Grid => self.render_grid(ctx, col_widths.as_slice(), focused),
            TableStyle::Clean => self.render_clean(ctx, col_widths.as_slice(), focused),
        };

        decorate_component_validation(&mut lines, ctx, self.base.id());

        DrawOutput { lines }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }

        let mut hints = vec![
            HintItem::new("Ctrl+F", "toggle filter", HintGroup::View).with_priority(30),
            HintItem::new("Enter", "submit step", HintGroup::Action).with_priority(40),
        ];

        if self.filter.is_focused() {
            hints.push(HintItem::new("Type", "filter rows", HintGroup::Edit).with_priority(10));
            hints.push(HintItem::new("Esc", "close filter", HintGroup::View).with_priority(11));
            return hints;
        }

        if self.move_mode {
            hints.push(HintItem::new("↑ ↓", "move row", HintGroup::Navigation).with_priority(10));
            hints
                .push(HintItem::new("m / Esc", "finish move", HintGroup::Action).with_priority(20));
            return hints;
        }

        if self.edit_mode {
            hints.push(
                HintItem::new("Tab / Shift+Tab", "next/prev column", HintGroup::Navigation)
                    .with_priority(10),
            );
            hints.push(HintItem::new("Esc", "finish edit", HintGroup::Action).with_priority(20));
            return hints;
        }

        match self.focus {
            TableFocus::Header => {
                hints.push(
                    HintItem::new("← → / Tab", "switch column", HintGroup::Navigation)
                        .with_priority(10),
                );
                hints.push(
                    HintItem::new("Space", "sort column", HintGroup::Action).with_priority(20),
                );
                hints.push(
                    HintItem::new("↓", "go to body", HintGroup::Navigation).with_priority(11),
                );
                hints.push(HintItem::new("i", "insert row", HintGroup::Action).with_priority(21));
            }
            TableFocus::Body => {
                hints.push(
                    HintItem::new("↑ ↓", "move rows", HintGroup::Navigation).with_priority(10),
                );
                hints.push(
                    HintItem::new("Tab / Shift+Tab", "switch column", HintGroup::Navigation)
                        .with_priority(11),
                );
                hints.push(HintItem::new("e", "edit cell", HintGroup::Action).with_priority(20));
                hints.push(
                    HintItem::new("i / d", "insert/delete row", HintGroup::Action)
                        .with_priority(21),
                );
                hints.push(HintItem::new("m", "move row", HintGroup::Action).with_priority(22));
            }
        }

        hints
    }
}

fn accent_active_cell(spans: &mut [Span]) {
    for span in spans {
        if span.style.color.is_none() {
            span.style.color = Some(Color::Cyan);
        }
        span.style.bold = true;
    }
}

fn span_line_text(spans: &[Span]) -> String {
    let mut out = String::new();
    for span in spans {
        out.push_str(span.text.as_str());
    }
    out
}

fn highlight_span_line(spans: &mut SpanLine, ranges: &[(usize, usize)], highlight: Style) {
    if ranges.is_empty() {
        return;
    }

    let mut sorted = ranges.to_vec();
    sorted.sort_unstable_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in sorted {
        if start >= end {
            continue;
        }
        if let Some((_, last_end)) = merged.last_mut()
            && start <= *last_end
        {
            *last_end = (*last_end).max(end);
            continue;
        }
        merged.push((start, end));
    }
    if merged.is_empty() {
        return;
    }

    let source = spans.clone();
    let mut out = Vec::<Span>::new();
    let mut global = 0usize;
    for span in source {
        let chars: Vec<char> = span.text.chars().collect();
        if chars.is_empty() {
            continue;
        }
        let mut idx = 0usize;
        while idx < chars.len() {
            let abs = global + idx;
            let marked = merged
                .iter()
                .any(|(start, end)| abs >= *start && abs < *end);
            let mut end_idx = idx + 1;
            while end_idx < chars.len() {
                let abs_next = global + end_idx;
                let marked_next = merged
                    .iter()
                    .any(|(start, end)| abs_next >= *start && abs_next < *end);
                if marked_next != marked {
                    break;
                }
                end_idx += 1;
            }
            let text: String = chars[idx..end_idx].iter().collect();
            let style = if marked {
                span.style.merge(highlight)
            } else {
                span.style
            };
            out.push(Span::styled(text, style).no_wrap());
            idx = end_idx;
        }
        global += chars.len();
    }

    if !out.is_empty() {
        *spans = out;
    }
}

fn grid_border_line(left: char, middle: char, right: char, widths: &[usize]) -> SpanLine {
    let border_style = Style::new().color(Color::DarkGrey);
    let mut line = Vec::<Span>::new();
    line.push(Span::styled(left.to_string(), border_style).no_wrap());
    for (idx, width) in widths.iter().enumerate() {
        line.push(Span::styled("─".repeat(width.saturating_add(2)), border_style).no_wrap());
        if idx + 1 < widths.len() {
            line.push(Span::styled(middle.to_string(), border_style).no_wrap());
        }
    }
    line.push(Span::styled(right.to_string(), border_style).no_wrap());
    line
}

fn grid_row(cells: Vec<SpanLine>, widths: &[usize]) -> SpanLine {
    let border_style = Style::new().color(Color::DarkGrey);
    let mut line = Vec::<Span>::new();
    for (idx, width) in widths.iter().enumerate() {
        line.push(Span::styled("│ ", border_style).no_wrap());
        let cell = cells.get(idx).cloned().unwrap_or_default();
        line.extend(Layout::fit_line(
            cell.as_slice(),
            (*width).min(u16::MAX as usize) as u16,
        ));
        line.push(Span::new(" ").no_wrap());
    }
    line.push(Span::styled("│", border_style).no_wrap());
    line
}

fn clean_row(cells: Vec<SpanLine>, widths: &[usize]) -> SpanLine {
    let mut line = Vec::<Span>::new();
    for (idx, width) in widths.iter().enumerate() {
        if idx > 0 {
            line.push(Span::new("  ").no_wrap());
        }
        let cell = cells.get(idx).cloned().unwrap_or_default();
        line.extend(Layout::fit_line(
            cell.as_slice(),
            (*width).min(u16::MAX as usize) as u16,
        ));
    }
    line
}

fn centered_label_line(text: &str, width: usize, style: Style) -> SpanLine {
    if width == 0 {
        return Vec::new();
    }
    let text_width = UnicodeWidthStr::width(text);
    if text_width >= width {
        return Layout::fit_line(
            &[Span::styled(text.to_string(), style).no_wrap()],
            width as u16,
        );
    }
    let left = (width - text_width) / 2;
    let right = width - text_width - left;
    vec![
        Span::new(" ".repeat(left)).no_wrap(),
        Span::styled(text.to_string(), style).no_wrap(),
        Span::new(" ".repeat(right)).no_wrap(),
    ]
}

fn grid_empty_row(widths: &[usize], text: &str) -> SpanLine {
    let border_style = Style::new().color(Color::DarkGrey);
    let text_style = Style::new().color(Color::DarkGrey);
    let border_width = Layout::line_width(grid_border_line('┌', '┬', '┐', widths).as_slice());
    let inner_width = border_width.saturating_sub(2);

    let mut line = vec![Span::styled("│".to_string(), border_style).no_wrap()];
    line.extend(centered_label_line(text, inner_width, text_style));
    line.push(Span::styled("│".to_string(), border_style).no_wrap());
    line
}

fn clean_empty_row(widths: &[usize], text: &str) -> SpanLine {
    let text_style = Style::new().color(Color::DarkGrey);
    let gaps = widths.len().saturating_sub(1).saturating_mul(2);
    let content_width: usize = widths.iter().copied().sum::<usize>().saturating_add(gaps);
    centered_label_line(text, content_width.max(1), text_style)
}
