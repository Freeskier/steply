use crate::core::value::Value;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, OutputNode, RenderContext};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOutputStyle {
    Grid,
    Clean,
}

pub struct TableOutput {
    id: String,
    label: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    style: TableOutputStyle,
}

impl TableOutput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            headers: Vec::new(),
            rows: Vec::new(),
            style: TableOutputStyle::Grid,
        }
    }

    pub fn with_style(mut self, style: TableOutputStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_headers(mut self, headers: Vec<String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_rows(mut self, rows: Vec<Vec<String>>) -> Self {
        self.rows = rows;
        self
    }

    pub fn set_headers(&mut self, headers: Vec<String>) {
        self.headers = headers;
    }

    pub fn set_rows(&mut self, rows: Vec<Vec<String>>) {
        self.rows = rows;
    }

    fn column_count(&self) -> usize {
        let from_rows = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        self.headers.len().max(from_rows)
    }

    fn normalized_headers(&self, cols: usize) -> Vec<String> {
        (0..cols)
            .map(|idx| {
                self.headers
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| format!("col{}", idx + 1))
            })
            .collect()
    }

    fn normalized_rows(&self, cols: usize) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|row| {
                (0..cols)
                    .map(|idx| row.get(idx).cloned().unwrap_or_default())
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn col_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<usize> {
        let cols = headers.len();
        let mut widths = headers
            .iter()
            .map(|h| UnicodeWidthStr::width(h.as_str()).max(3))
            .collect::<Vec<_>>();
        if widths.len() < cols {
            widths.resize(cols, 3);
        }
        for row in rows {
            for (idx, cell) in row.iter().enumerate().take(cols) {
                widths[idx] = widths[idx].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
        widths
    }

    fn render_grid(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        widths: &[usize],
    ) -> DrawOutput {
        let mut lines = Vec::<Vec<Span>>::new();
        if !self.label.is_empty() {
            lines.push(vec![Span::new(self.label.clone()).no_wrap()]);
        }

        lines.push(border_line('┌', '┬', '┐', widths));
        lines.push(grid_row(
            headers
                .iter()
                .map(|h| {
                    vec![Span::styled(h.clone(), Style::new().color(Color::Cyan).bold()).no_wrap()]
                })
                .collect(),
            widths,
        ));
        lines.push(border_line('├', '┼', '┤', widths));

        for (idx, row) in rows.iter().enumerate() {
            lines.push(grid_row(
                row.iter()
                    .map(|cell| vec![Span::new(cell.clone()).no_wrap()])
                    .collect(),
                widths,
            ));
            if idx + 1 < rows.len() {
                lines.push(border_line('├', '┼', '┤', widths));
            }
        }

        lines.push(border_line('└', '┴', '┘', widths));
        DrawOutput { lines }
    }

    fn render_clean(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        widths: &[usize],
    ) -> DrawOutput {
        let mut lines = Vec::<Vec<Span>>::new();
        if !self.label.is_empty() {
            lines.push(vec![Span::new(self.label.clone()).no_wrap()]);
        }

        lines.push(clean_row(
            headers
                .iter()
                .map(|h| {
                    vec![Span::styled(h.clone(), Style::new().color(Color::Cyan).bold()).no_wrap()]
                })
                .collect(),
            widths,
        ));

        for row in rows {
            lines.push(clean_row(
                row.iter()
                    .map(|cell| vec![Span::new(cell.clone()).no_wrap()])
                    .collect(),
                widths,
            ));
        }

        DrawOutput { lines }
    }
}

impl Drawable for TableOutput {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let cols = self.column_count();
        if cols == 0 {
            let mut lines = Vec::<Vec<Span>>::new();
            if !self.label.is_empty() {
                lines.push(vec![Span::new(self.label.clone()).no_wrap()]);
            }
            lines.push(vec![
                Span::styled("No rows", Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
            return DrawOutput { lines };
        }

        let headers = self.normalized_headers(cols);
        let rows = self.normalized_rows(cols);
        let widths = Self::col_widths(headers.as_slice(), rows.as_slice());

        match self.style {
            TableOutputStyle::Grid => {
                self.render_grid(headers.as_slice(), rows.as_slice(), widths.as_slice())
            }
            TableOutputStyle::Clean => {
                self.render_clean(headers.as_slice(), rows.as_slice(), widths.as_slice())
            }
        }
    }
}

impl OutputNode for TableOutput {
    fn set_value(&mut self, value: Value) {
        match value {
            Value::None => {
                self.rows.clear();
            }
            Value::List(list) => {
                if list.iter().all(|v| matches!(v, Value::Object(_))) {
                    apply_object_rows(self, list.as_slice());
                } else {
                    apply_list_rows(self, list.as_slice());
                }
            }
            Value::Object(obj) => {
                apply_object_rows(self, &[Value::Object(obj)]);
            }
            scalar => {
                self.rows = vec![vec![scalar.to_text_scalar().unwrap_or_default()]];
                if self.headers.is_empty() {
                    self.headers = vec!["value".to_string()];
                }
            }
        }
    }
}

fn apply_list_rows(table: &mut TableOutput, list: &[Value]) {
    let rows = list
        .iter()
        .map(|item| match item {
            Value::List(cells) => cells
                .iter()
                .map(|cell| cell.to_text_scalar().unwrap_or_else(|| cell.to_json()))
                .collect::<Vec<_>>(),
            _ => vec![item.to_text_scalar().unwrap_or_else(|| item.to_json())],
        })
        .collect::<Vec<_>>();
    table.rows = rows;
    if table.headers.is_empty() {
        let cols = table.column_count();
        table.headers = (0..cols).map(|idx| format!("col{}", idx + 1)).collect();
    }
}

fn apply_object_rows(table: &mut TableOutput, list: &[Value]) {
    let mut headers = table.headers.clone();
    if headers.is_empty() {
        for item in list {
            if let Value::Object(map) = item {
                for key in map.keys() {
                    if !headers.iter().any(|h| h == key) {
                        headers.push(key.clone());
                    }
                }
            }
        }
    }

    let rows = list
        .iter()
        .filter_map(|item| {
            let Value::Object(map) = item else {
                return None;
            };
            Some(
                headers
                    .iter()
                    .map(|header| {
                        map.get(header)
                            .and_then(Value::to_text_scalar)
                            .unwrap_or_else(|| {
                                map.get(header).map(Value::to_json).unwrap_or_default()
                            })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    table.headers = headers;
    table.rows = rows;
}

fn border_line(left: char, sep: char, right: char, widths: &[usize]) -> Vec<Span> {
    let mut text = String::new();
    text.push(left);
    for (idx, width) in widths.iter().enumerate() {
        text.push_str(&"─".repeat(*width + 2));
        if idx + 1 < widths.len() {
            text.push(sep);
        }
    }
    text.push(right);
    vec![Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap()]
}

fn grid_row(cells: Vec<Vec<Span>>, widths: &[usize]) -> Vec<Span> {
    let mut line = Vec::<Span>::new();
    line.push(Span::styled("│", Style::new().color(Color::DarkGrey)).no_wrap());
    for (idx, width) in widths.iter().enumerate() {
        line.push(Span::new(" ").no_wrap());
        let cell = cells
            .get(idx)
            .cloned()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()]);
        let used = span_line_width(cell.as_slice());
        line.extend(cell);
        if *width > used {
            line.push(Span::new(" ".repeat(*width - used)).no_wrap());
        }
        line.push(Span::new(" ").no_wrap());
        line.push(Span::styled("│", Style::new().color(Color::DarkGrey)).no_wrap());
    }
    line
}

fn clean_row(cells: Vec<Vec<Span>>, widths: &[usize]) -> Vec<Span> {
    let mut line = Vec::<Span>::new();
    for (idx, width) in widths.iter().enumerate() {
        if idx > 0 {
            line.push(Span::styled("  ", Style::new().color(Color::DarkGrey)).no_wrap());
        }
        let cell = cells
            .get(idx)
            .cloned()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()]);
        let used = span_line_width(cell.as_slice());
        line.extend(cell);
        if *width > used {
            line.push(Span::new(" ".repeat(*width - used)).no_wrap());
        }
    }
    line
}

fn span_line_width(line: &[Span]) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.text.as_str()))
        .sum()
}
