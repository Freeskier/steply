use crate::core::binding::BindTarget;
use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::inputs::Input;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine, RenderOutput};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

pub type TableInputFactory = Arc<dyn Fn(String) -> Box<dyn Input> + Send + Sync>;

#[derive(Clone)]
pub struct TableColumn {
    pub id: String,
    pub label: String,
    pub min_width: usize,
    input_factory: TableInputFactory,
}

impl TableColumn {
    pub fn new<F>(id: impl Into<String>, label: impl Into<String>, factory: F) -> Self
    where
        F: Fn(String) -> Box<dyn Input> + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            label: label.into(),
            min_width: 6,
            input_factory: Arc::new(factory),
        }
    }

    pub fn with_min_width(mut self, width: usize) -> Self {
        self.min_width = width.max(1);
        self
    }

    fn build_input(&self, input_id: String) -> Box<dyn Input> {
        (self.input_factory)(input_id)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TableBorders {
    pub outer: bool,
    pub between_cells: bool,
}

impl TableBorders {
    pub fn none() -> Self {
        Self {
            outer: false,
            between_cells: false,
        }
    }

    pub fn full() -> Self {
        Self {
            outer: true,
            between_cells: true,
        }
    }
}

impl Default for TableBorders {
    fn default() -> Self {
        Self::none()
    }
}

pub struct TableComponent {
    base: ComponentBase,
    title: Option<String>,
    columns: Vec<TableColumn>,
    rows: Vec<Vec<Box<dyn Input>>>,
    active_row: usize,
    active_col: usize,
    next_cell_id: u64,
    borders: TableBorders,
    bind_target: Option<BindTarget>,
    header_style: Style,
    row_style: Style,
    active_row_style: Style,
    border_style: Style,
}

impl TableComponent {
    pub fn new(id: impl Into<String>, columns: Vec<TableColumn>) -> Self {
        let columns = if columns.is_empty() {
            vec![TableColumn::new("value", "Value", |input_id| {
                Box::new(crate::text_input::TextInput::new(input_id, ""))
            })]
        } else {
            columns
        };

        let mut component = Self {
            base: ComponentBase::new(id),
            title: None,
            columns,
            rows: Vec::new(),
            active_row: 0,
            active_col: 0,
            next_cell_id: 1,
            borders: TableBorders::none(),
            bind_target: None,
            header_style: Style::new().with_color(Color::Cyan).with_bold(),
            row_style: Style::new().with_color(Color::DarkGrey),
            active_row_style: Style::new().with_color(Color::Green).with_bold(),
            border_style: Style::new().with_color(Color::DarkGrey),
        };
        component.ensure_row_count(1);
        component
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_row_count(mut self, count: usize) -> Self {
        self.ensure_row_count(count.max(1));
        self
    }

    pub fn with_cell_value(mut self, row: usize, col_id: &str, value: Value) -> Self {
        let _ = self.set_cell_value(row, col_id, value);
        self
    }

    pub fn set_cell_value(&mut self, row: usize, col_id: &str, value: Value) -> bool {
        let Some(col_idx) = self.columns.iter().position(|col| col.id == col_id) else {
            return false;
        };
        if row >= self.rows.len() {
            self.ensure_row_count(row + 1);
        }
        self.rows[row][col_idx].set_value_typed(value);
        true
    }

    pub fn with_borders(mut self, borders: TableBorders) -> Self {
        self.borders = borders;
        self
    }

    pub fn with_outer_border(mut self, enabled: bool) -> Self {
        self.borders.outer = enabled;
        self
    }

    pub fn with_cell_borders(mut self, enabled: bool) -> Self {
        self.borders.between_cells = enabled;
        self
    }

    pub fn with_bind_target(mut self, target: BindTarget) -> Self {
        self.bind_target = Some(target);
        self
    }

    pub fn bind_to_input(mut self, id: impl Into<String>) -> Self {
        self.bind_target = Some(BindTarget::Input(id.into()));
        self
    }

    pub fn with_header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    pub fn with_row_style(mut self, style: Style) -> Self {
        self.row_style = style;
        self
    }

    pub fn with_active_row_style(mut self, style: Style) -> Self {
        self.active_row_style = style;
        self
    }

    pub fn with_border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    fn alloc_cell_id(&mut self, col_id: &str) -> String {
        let id = format!("{}_{}_{}", self.base.id, col_id, self.next_cell_id);
        self.next_cell_id += 1;
        id
    }

    fn build_row(&mut self) -> Vec<Box<dyn Input>> {
        let mut row = Vec::with_capacity(self.columns.len());
        for col_idx in 0..self.columns.len() {
            let col_id = self.columns[col_idx].id.clone();
            let input_id = self.alloc_cell_id(&col_id);
            row.push(self.columns[col_idx].build_input(input_id));
        }
        row
    }

    fn ensure_row_count(&mut self, count: usize) {
        while self.rows.len() < count {
            let row = self.build_row();
            self.rows.push(row);
        }
        if self.rows.len() > count {
            self.rows.truncate(count);
        }
        self.clamp_active();
        self.apply_focus();
    }

    fn clamp_active(&mut self) {
        if self.rows.is_empty() {
            self.active_row = 0;
        } else {
            self.active_row = self.active_row.min(self.rows.len() - 1);
        }
        if self.columns.is_empty() {
            self.active_col = 0;
        } else {
            self.active_col = self.active_col.min(self.columns.len() - 1);
        }
    }

    fn apply_focus(&mut self) {
        for row in &mut self.rows {
            for cell in row {
                cell.set_focused(false);
            }
        }
        if self.base.focused
            && !self.rows.is_empty()
            && self.active_row < self.rows.len()
            && self.active_col < self.columns.len()
        {
            self.rows[self.active_row][self.active_col].set_focused(true);
        }
    }

    fn active_input(&self) -> &dyn Input {
        self.rows[self.active_row][self.active_col].as_ref()
    }

    fn active_input_mut(&mut self) -> &mut dyn Input {
        self.rows[self.active_row][self.active_col].as_mut()
    }

    fn move_row(&mut self, delta: isize) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let len = self.rows.len() as isize;
        let current = self.active_row as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
        if next == self.active_row {
            return false;
        }
        self.active_row = next;
        self.apply_focus();
        true
    }

    fn move_col(&mut self, delta: isize) -> bool {
        if self.columns.is_empty() {
            return false;
        }
        let len = self.columns.len() as isize;
        let current = self.active_col as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
        if next == self.active_col {
            return false;
        }
        self.active_col = next;
        self.apply_focus();
        true
    }

    fn move_cell_next(&mut self) {
        let cols = self.columns.len();
        let rows = self.rows.len();
        if cols == 0 || rows == 0 {
            return;
        }

        if self.active_col + 1 < cols {
            self.active_col += 1;
        } else {
            self.active_col = 0;
            self.active_row = (self.active_row + 1) % rows;
        }
        self.apply_focus();
    }

    fn move_cell_prev(&mut self) {
        let cols = self.columns.len();
        let rows = self.rows.len();
        if cols == 0 || rows == 0 {
            return;
        }

        if self.active_col > 0 {
            self.active_col -= 1;
        } else {
            self.active_col = cols - 1;
            if self.active_row == 0 {
                self.active_row = rows - 1;
            } else {
                self.active_row -= 1;
            }
        }
        self.apply_focus();
    }

    fn add_row_after_active(&mut self) -> bool {
        let insert_at = (self.active_row + 1).min(self.rows.len());
        let row = self.build_row();
        self.rows.insert(insert_at, row);
        self.active_row = insert_at;
        self.active_col = self.active_col.min(self.columns.len().saturating_sub(1));
        self.apply_focus();
        true
    }

    fn remove_active_row(&mut self) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        self.rows.remove(self.active_row);
        if self.rows.is_empty() {
            self.ensure_row_count(1);
            self.active_row = 0;
        } else if self.active_row >= self.rows.len() {
            self.active_row = self.rows.len() - 1;
        }
        self.apply_focus();
        true
    }

    fn value_to_plain(value: Value) -> String {
        match value {
            Value::None => "·".to_string(),
            Value::Text(text) => {
                if text.is_empty() {
                    "·".to_string()
                } else {
                    text
                }
            }
            Value::Bool(flag) => {
                if flag {
                    "x".to_string()
                } else {
                    "·".to_string()
                }
            }
            Value::Number(num) => num.to_string(),
            Value::List(items) => {
                if items.is_empty() {
                    "·".to_string()
                } else {
                    items.join(",")
                }
            }
            Value::Map(items) => {
                if items.is_empty() {
                    "·".to_string()
                } else {
                    format!("{} items", items.len())
                }
            }
        }
    }

    fn value_to_store_string(value: Value) -> String {
        match value {
            Value::None => String::new(),
            Value::Text(text) => text,
            Value::Bool(flag) => flag.to_string(),
            Value::Number(num) => num.to_string(),
            Value::List(items) => items.join(","),
            Value::Map(items) => items
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(","),
        }
    }

    fn preview_text(&self, row: usize, col: usize) -> String {
        let value = self.rows[row][col].value_typed();
        Self::value_to_plain(value)
    }

    fn active_input_render(&self, ctx: &RenderContext) -> (String, usize) {
        let output = ctx.render_input_field(self.active_input(), false, true);
        let mut text = String::new();
        for line in &output.lines {
            for span in &line.spans {
                text.push_str(span.text());
            }
        }
        let cursor = output
            .cursor
            .map(|c| c.offset)
            .unwrap_or_else(|| text.width());
        (text, cursor)
    }

    fn pad_to_width(text: &str, width: usize) -> String {
        let current = text.width();
        if current >= width {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + (width - current));
        out.push_str(text);
        out.push_str(&" ".repeat(width - current));
        out
    }

    fn compute_column_widths(&self, ctx: &RenderContext) -> Vec<usize> {
        let mut widths = self
            .columns
            .iter()
            .map(|col| col.label.width().max(col.min_width))
            .collect::<Vec<_>>();

        for row_idx in 0..self.rows.len() {
            for col_idx in 0..self.columns.len() {
                let plain = self.preview_text(row_idx, col_idx);
                widths[col_idx] = widths[col_idx].max(plain.width());
            }
        }

        if !self.rows.is_empty() {
            let (active, _) = self.active_input_render(ctx);
            widths[self.active_col] = widths[self.active_col].max(active.width());
        }

        widths
    }

    fn row_number_width(&self) -> usize {
        self.rows.len().max(1).to_string().len()
    }

    fn build_row_prefix(&self, row_idx: usize, active: bool) -> String {
        let width = self.row_number_width();
        if active {
            format!("❯ {:>width$} ", row_idx + 1, width = width)
        } else {
            format!("  {:>width$} ", row_idx + 1, width = width)
        }
    }

    fn build_header_prefix(&self) -> String {
        let width = self.row_number_width();
        format!("  {:width$} ", "", width = width)
    }

    fn cell_separator(&self) -> &'static str {
        if self.borders.between_cells {
            " │ "
        } else {
            "  "
        }
    }

    fn compose_header_core(&self, widths: &[usize]) -> String {
        let mut line = self.build_header_prefix();
        let sep = self.cell_separator();
        for (idx, col) in self.columns.iter().enumerate() {
            if idx > 0 {
                line.push_str(sep);
            }
            line.push_str(&Self::pad_to_width(&col.label, widths[idx]));
        }
        line
    }

    fn core_vertical_positions(&self, widths: &[usize]) -> Vec<usize> {
        if !self.borders.between_cells {
            return Vec::new();
        }

        let core = self.compose_header_core(widths);
        let mut out = Vec::new();
        let mut width = 0usize;
        for ch in core.chars() {
            if ch == '│' {
                out.push(width);
            }
            width += ch.to_string().width();
        }
        out
    }

    fn compose_data_core(
        &self,
        row_idx: usize,
        widths: &[usize],
        active_text: &str,
    ) -> (String, Option<usize>) {
        let active_row = self.base.focused && row_idx == self.active_row;
        let mut line = self.build_row_prefix(row_idx, active_row);
        let sep = self.cell_separator();
        let mut cursor = None;

        for col_idx in 0..self.columns.len() {
            if col_idx > 0 {
                line.push_str(sep);
            }

            if active_row && col_idx == self.active_col {
                let start = line.width();
                cursor = Some(start);
                line.push_str(&Self::pad_to_width(active_text, widths[col_idx]));
            } else {
                let plain = self.preview_text(row_idx, col_idx);
                line.push_str(&Self::pad_to_width(&plain, widths[col_idx]));
            }
        }

        (line, cursor)
    }

    fn wrap_border_line(&self, core: String) -> (String, usize) {
        if self.borders.outer {
            (format!("│ {} │", core), 2)
        } else {
            (core, 0)
        }
    }

    fn border_line(
        &self,
        core_width: usize,
        core_vertical_positions: &[usize],
        left: char,
        junction: char,
        right: char,
    ) -> Option<String> {
        if !self.borders.outer {
            return None;
        }

        let total = core_width + 4;
        let mut chars = vec!['─'; total];
        chars[0] = left;
        chars[total - 1] = right;
        for pos in core_vertical_positions {
            let idx = 2 + *pos;
            if idx > 0 && idx < total - 1 {
                chars[idx] = junction;
            }
        }

        Some(chars.into_iter().collect())
    }

    fn top_border(&self, core_width: usize, core_vertical_positions: &[usize]) -> Option<String> {
        self.border_line(core_width, core_vertical_positions, '┌', '┬', '┐')
    }

    fn header_divider(
        &self,
        core_width: usize,
        core_vertical_positions: &[usize],
    ) -> Option<String> {
        self.border_line(core_width, core_vertical_positions, '├', '┼', '┤')
    }

    fn bottom_border(
        &self,
        core_width: usize,
        core_vertical_positions: &[usize],
    ) -> Option<String> {
        self.border_line(core_width, core_vertical_positions, '└', '┴', '┘')
    }

    fn styled_header_spans(&self, text: &str) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut current = String::new();
        let mut is_border = false;
        let mut has_current = false;

        for ch in text.chars() {
            let next_is_border = ch == '│';
            if !has_current {
                current.push(ch);
                is_border = next_is_border;
                has_current = true;
                continue;
            }

            if next_is_border == is_border {
                current.push(ch);
            } else {
                let style = if is_border {
                    self.border_style.clone()
                } else {
                    self.header_style.clone()
                };
                spans.push(Span::new(std::mem::take(&mut current)).with_style(style));
                current.push(ch);
                is_border = next_is_border;
            }
        }

        if !current.is_empty() {
            let style = if is_border {
                self.border_style.clone()
            } else {
                self.header_style.clone()
            };
            spans.push(Span::new(current).with_style(style));
        }

        spans
    }

    fn map_value(&self) -> Value {
        let mut out = Vec::new();
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (col_idx, col) in self.columns.iter().enumerate() {
                let key = format!("row{}.{}", row_idx + 1, col.id);
                let value = Self::value_to_store_string(row[col_idx].value_typed());
                out.push((key, value));
            }
        }
        Value::Map(out)
    }

    fn restore_from_map(&mut self, entries: Vec<(String, String)>) {
        let mut max_row = 0usize;
        let mut parsed = Vec::new();

        for (key, value) in entries {
            let Some((row_part, col_id)) = key.split_once('.') else {
                continue;
            };
            let Some(row_num_str) = row_part.strip_prefix("row") else {
                continue;
            };
            let Ok(row_num) = row_num_str.parse::<usize>() else {
                continue;
            };
            if row_num == 0 {
                continue;
            }
            let Some(col_idx) = self.columns.iter().position(|col| col.id == col_id) else {
                continue;
            };

            max_row = max_row.max(row_num);
            parsed.push((row_num - 1, col_idx, value));
        }

        if max_row == 0 {
            return;
        }

        self.ensure_row_count(max_row);
        for (row_idx, col_idx, value) in parsed {
            if row_idx < self.rows.len() && col_idx < self.columns.len() {
                self.rows[row_idx][col_idx].set_value(value);
            }
        }
        self.apply_focus();
    }
}

impl Component for TableComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn bind_target(&self) -> Option<BindTarget> {
        self.bind_target.clone()
    }

    fn render(&self, ctx: &RenderContext) -> RenderOutput {
        let widths = self.compute_column_widths(ctx);
        let header_core = self.compose_header_core(&widths);
        let core_width = header_core.width();
        let core_vertical_positions = self.core_vertical_positions(&widths);

        let (active_text, active_cursor_inner) = self.active_input_render(ctx);

        let mut lines = Vec::new();
        let mut cursor = None;

        if let Some(title) = &self.title {
            lines.push(RenderLine {
                spans: vec![Span::new(title.clone()).with_style(self.header_style.clone())],
            });
        }

        if let Some(top) = self.top_border(core_width, &core_vertical_positions) {
            lines.push(RenderLine {
                spans: vec![Span::new(top).with_style(self.border_style.clone())],
            });
        }

        let (header_text, _) = self.wrap_border_line(header_core);
        lines.push(RenderLine {
            spans: self.styled_header_spans(&header_text),
        });

        if let Some(divider) = self.header_divider(core_width, &core_vertical_positions) {
            lines.push(RenderLine {
                spans: vec![Span::new(divider).with_style(self.border_style.clone())],
            });
        }

        for row_idx in 0..self.rows.len() {
            let (row_core, row_cursor) = self.compose_data_core(row_idx, &widths, &active_text);
            let (row_text, cursor_shift) = self.wrap_border_line(row_core);
            let is_active = self.base.focused && row_idx == self.active_row;
            let style = if is_active {
                self.active_row_style.clone()
            } else {
                self.row_style.clone()
            };
            let line_idx = lines.len();
            lines.push(RenderLine {
                spans: vec![Span::new(row_text).with_style(style)],
            });

            if is_active {
                let base_offset = row_cursor.unwrap_or(0) + cursor_shift;
                cursor = Some((line_idx, base_offset + active_cursor_inner));
            }
        }

        if let Some(bottom) = self.bottom_border(core_width, &core_vertical_positions) {
            lines.push(RenderLine {
                spans: vec![Span::new(bottom).with_style(self.border_style.clone())],
            });
        }

        let mut output = RenderOutput::from_lines(lines);
        if let Some((line, offset)) = cursor {
            output = output.with_cursor(line, offset);
        }
        output
    }

    fn value(&self) -> Option<Value> {
        Some(self.map_value())
    }

    fn set_value(&mut self, value: Value) {
        if let Value::Map(entries) = value {
            self.restore_from_map(entries);
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        if modifiers == KeyModifiers::CONTROL {
            let handled = match code {
                KeyCode::Char('a') => self.add_row_after_active(),
                KeyCode::Char('d') => self.remove_active_row(),
                KeyCode::Char('s') => {
                    if let Some(value) = self.value() {
                        return ComponentResponse::produced(value);
                    }
                    false
                }
                _ => false,
            };

            return if handled {
                ComponentResponse::handled()
            } else {
                ComponentResponse::not_handled()
            };
        }

        if modifiers == KeyModifiers::SHIFT && matches!(code, KeyCode::Tab | KeyCode::BackTab) {
            self.move_cell_prev();
            return ComponentResponse::handled();
        }

        if modifiers == KeyModifiers::NONE && matches!(code, KeyCode::Tab | KeyCode::BackTab) {
            self.move_cell_next();
            return ComponentResponse::handled();
        }

        let input_result = {
            let input = self.active_input_mut();
            input.handle_key(code, modifiers)
        };

        match input_result {
            crate::inputs::KeyResult::Handled => {
                return ComponentResponse::handled();
            }
            crate::inputs::KeyResult::Submit => {
                self.move_cell_next();
                return ComponentResponse::handled();
            }
            crate::inputs::KeyResult::NotHandled => {}
        }

        if modifiers != KeyModifiers::NONE {
            return ComponentResponse::not_handled();
        }

        let handled = match code {
            KeyCode::Up => self.move_row(-1),
            KeyCode::Down => self.move_row(1),
            KeyCode::Left => self.move_col(-1),
            KeyCode::Right => self.move_col(1),
            _ => false,
        };

        if handled {
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    fn delete_word(&mut self) -> ComponentResponse {
        let before = self.active_input().value();
        self.active_input_mut().delete_word();
        let after = self.active_input().value();
        if before != after {
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    fn delete_word_forward(&mut self) -> ComponentResponse {
        let before = self.active_input().value();
        self.active_input_mut().delete_word_forward();
        let after = self.active_input().value();
        if before != after {
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.apply_focus();
    }
}
