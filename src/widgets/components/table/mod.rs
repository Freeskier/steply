use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::core::search::fuzzy::match_text;
use crate::core::value::Value;
use crate::runtime::event::WidgetAction;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionMenu, CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult,
    Interactive, InteractiveNode, RenderContext, TextAction, ValidationMode,
};

pub type CellFactory = Arc<dyn Fn(String, String) -> Box<dyn InteractiveNode> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStyle {
    Grid,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableFocus {
    Header,
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Asc,
    Desc,
}

struct ColumnDef {
    header: String,
    key: String,
    min_width: usize,
    make_cell: CellFactory,
}

struct RowState {
    id: u64,
    cells: Vec<Box<dyn InteractiveNode>>,
}

pub struct Table {
    base: WidgetBase,
    style: TableStyle,
    show_row_numbers: bool,
    columns: Vec<ColumnDef>,
    rows: Vec<RowState>,
    focus: TableFocus,
    active_row: usize,
    active_col: usize,
    move_mode: bool,
    edit_mode: bool,
    filter: TextInput,
    filter_visible: bool,
    filter_focus: bool,
    visible_rows: Vec<usize>,
    sort: Option<(usize, SortDirection)>,
    next_row_id: u64,
}

impl Table {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let label = label.into();
        let mut this = Self {
            base: WidgetBase::new(id.clone(), label),
            style: TableStyle::Grid,
            show_row_numbers: true,
            columns: Vec::new(),
            rows: Vec::new(),
            focus: TableFocus::Header,
            active_row: 0,
            active_col: 0,
            move_mode: false,
            edit_mode: false,
            filter: TextInput::new(format!("{id}__filter"), ""),
            filter_visible: false,
            filter_focus: false,
            visible_rows: Vec::new(),
            sort: None,
            next_row_id: 1,
        };
        this.apply_filter(None);
        this
    }

    pub fn with_style(mut self, style: TableStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_row_numbers(mut self, show_row_numbers: bool) -> Self {
        self.show_row_numbers = show_row_numbers;
        self
    }

    pub fn with_initial_rows(mut self, rows: usize) -> Self {
        for _ in 0..rows {
            self.add_row();
        }
        self
    }

    pub fn column<I, F>(mut self, header: impl Into<String>, make_cell: F) -> Self
    where
        I: InteractiveNode + 'static,
        F: Fn(String, String) -> I + Send + Sync + 'static,
    {
        self.push_column(
            header.into(),
            Arc::new(move |id, label| Box::new(make_cell(id, label))),
        );
        self
    }

    pub fn column_boxed(mut self, header: impl Into<String>, make_cell: CellFactory) -> Self {
        self.push_column(header.into(), make_cell);
        self
    }

    pub fn add_row(&mut self) -> usize {
        let row_id = self.next_row_id;
        self.next_row_id = self.next_row_id.saturating_add(1);
        let row = self.build_row(row_id, None);
        self.rows.push(row);
        self.apply_sort_preserving_focus(Some(row_id));

        self.focus = TableFocus::Body;
        if let Some(pos) = self.rows.iter().position(|r| r.id == row_id) {
            self.active_row = pos;
        }
        self.active_col = self.active_col.min(self.columns.len().saturating_sub(1));
        self.active_row
    }

    fn insert_row_after_active(&mut self) {
        if self.rows.is_empty() {
            self.add_row();
            return;
        }
        let row_id = self.next_row_id;
        self.next_row_id = self.next_row_id.saturating_add(1);
        let row = self.build_row(row_id, None);
        let insert_at = self.active_row.saturating_add(1).min(self.rows.len());
        self.rows.insert(insert_at, row);
        self.sort = None;
        self.focus = TableFocus::Body;
        self.active_row = insert_at;
        self.active_col = self.active_col.min(self.columns.len().saturating_sub(1));
        self.edit_mode = true;
        self.move_mode = false;
        self.apply_filter(Some(row_id));
    }

    fn delete_active_row(&mut self) {
        if self.rows.is_empty() || self.focus != TableFocus::Body {
            return;
        }
        let preferred = if self.rows.len() > 1 {
            self.rows
                .get(self.active_row.saturating_sub(1))
                .map(|row| row.id)
        } else {
            None
        };
        self.rows.remove(self.active_row);
        self.move_mode = false;
        if self.rows.is_empty() {
            self.focus = TableFocus::Header;
            self.active_row = 0;
            self.edit_mode = false;
        } else {
            self.active_row = self.active_row.min(self.rows.len().saturating_sub(1));
        }
        self.apply_filter(preferred);
    }

    fn move_active_row_by(&mut self, delta: isize) -> bool {
        if self.rows.len() < 2 || self.focus != TableFocus::Body {
            return false;
        }
        let Some(next_idx) = self.active_row.checked_add_signed(delta) else {
            return false;
        };
        if next_idx >= self.rows.len() {
            return false;
        }
        self.sort = None;
        let active_id = self.rows.get(self.active_row).map(|row| row.id);
        self.rows.swap(self.active_row, next_idx);
        self.active_row = next_idx;
        self.apply_filter(active_id);
        true
    }

    fn push_column(&mut self, header: String, make_cell: CellFactory) {
        let key = self.unique_column_key(header.as_str());
        let min_width = UnicodeWidthStr::width(header.as_str()).max(6);
        self.columns.push(ColumnDef {
            header,
            key,
            min_width,
            make_cell: make_cell.clone(),
        });

        let col_idx = self.columns.len().saturating_sub(1);
        for row in &mut self.rows {
            let cell_id = format!("{}__r{}__c{}", self.base.id(), row.id, col_idx);
            row.cells
                .push(make_cell(cell_id, self.columns[col_idx].header.clone()));
        }
        self.clamp_focus();
        self.apply_filter(self.active_row_id());
    }

    fn unique_column_key(&self, header: &str) -> String {
        let base = normalize_key(header);
        if !self.columns.iter().any(|c| c.key == base) {
            return base;
        }
        let mut idx = 2usize;
        loop {
            let key = format!("{base}_{idx}");
            if !self.columns.iter().any(|c| c.key == key) {
                return key;
            }
            idx = idx.saturating_add(1);
        }
    }

    fn build_row(&self, row_id: u64, seed: Option<&Value>) -> RowState {
        let mut cells = Vec::<Box<dyn InteractiveNode>>::with_capacity(self.columns.len());
        for (col_idx, col) in self.columns.iter().enumerate() {
            let cell_id = format!("{}__r{}__c{}", self.base.id(), row_id, col_idx);
            let mut cell = (col.make_cell)(cell_id, col.header.clone());
            if let Some(value) = seed_value(seed, col_idx, col.key.as_str(), col.header.as_str()) {
                cell.set_value(value);
            }
            cells.push(cell);
        }
        RowState { id: row_id, cells }
    }

    fn toggle_sort(&mut self, col_idx: usize) {
        if col_idx >= self.columns.len() {
            return;
        }

        self.sort = match self.sort {
            Some((idx, SortDirection::Asc)) if idx == col_idx => Some((idx, SortDirection::Desc)),
            Some((idx, SortDirection::Desc)) if idx == col_idx => None,
            _ => Some((col_idx, SortDirection::Asc)),
        };
        self.apply_sort_preserving_focus(self.active_row_id());
    }

    fn apply_sort_preserving_focus(&mut self, focused_row_id: Option<u64>) {
        match self.sort {
            Some((col_idx, direction)) => {
                self.rows.sort_by(|left, right| {
                    let ordering = compare_cell_values(
                        left.cells.get(col_idx).and_then(|cell| cell.value()),
                        right.cells.get(col_idx).and_then(|cell| cell.value()),
                    );
                    let ordering = match direction {
                        SortDirection::Asc => ordering,
                        SortDirection::Desc => ordering.reverse(),
                    };
                    if ordering == Ordering::Equal {
                        left.id.cmp(&right.id)
                    } else {
                        ordering
                    }
                });
            }
            None => self.rows.sort_by_key(|row| row.id),
        }

        if let Some(id) = focused_row_id
            && let Some(pos) = self.rows.iter().position(|row| row.id == id)
        {
            self.active_row = pos;
        }
        self.clamp_focus();
        self.apply_filter(focused_row_id);
    }

    fn active_row_id(&self) -> Option<u64> {
        if self.focus != TableFocus::Body {
            return None;
        }
        self.rows.get(self.active_row).map(|row| row.id)
    }

    fn clamp_focus(&mut self) {
        if self.columns.is_empty() {
            self.active_col = 0;
        } else {
            self.active_col = self.active_col.min(self.columns.len().saturating_sub(1));
        }

        if self.rows.is_empty() {
            self.active_row = 0;
            self.move_mode = false;
            self.edit_mode = false;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        self.active_row = self.active_row.min(self.rows.len().saturating_sub(1));
    }

    fn toggle_filter_visibility(&mut self) {
        self.filter_visible = !self.filter_visible;
        if self.filter_visible {
            self.filter_focus = true;
            return;
        }
        self.filter_focus = false;
        self.filter.set_value(Value::Text(String::new()));
        self.apply_filter(self.active_row_id());
    }

    fn filter_query(&self) -> String {
        self.filter
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    fn cell_filter_text(&self, row_idx: usize, col_idx: usize) -> String {
        self.rows
            .get(row_idx)
            .and_then(|row| row.cells.get(col_idx))
            .and_then(|cell| cell.value())
            .map(|value| value_sort_text(&value))
            .unwrap_or_default()
    }

    fn apply_filter(&mut self, preferred_row_id: Option<u64>) {
        let query = self.filter_query();
        let query = query.trim();

        self.visible_rows.clear();
        if query.is_empty() {
            self.visible_rows.extend(0..self.rows.len());
        } else {
            for row_idx in 0..self.rows.len() {
                let matched = (0..self.columns.len()).any(|col_idx| {
                    let text = self.cell_filter_text(row_idx, col_idx);
                    match_text(query, text.as_str()).is_some()
                });
                if matched {
                    self.visible_rows.push(row_idx);
                }
            }
        }

        if self.rows.is_empty() {
            self.active_row = 0;
            self.move_mode = false;
            self.edit_mode = false;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        if self.visible_rows.is_empty() {
            self.move_mode = false;
            self.edit_mode = false;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        if let Some(id) = preferred_row_id
            && let Some(row_idx) = self.rows.iter().position(|row| row.id == id)
            && self.visible_rows.contains(&row_idx)
        {
            self.active_row = row_idx;
            return;
        }

        if !self.visible_rows.contains(&self.active_row) {
            self.active_row = self.visible_rows[0];
        }
    }

    fn active_visible_pos(&self) -> Option<usize> {
        self.visible_rows
            .iter()
            .position(|row_idx| *row_idx == self.active_row)
    }

    fn move_active_visible(&mut self, delta: isize) -> bool {
        let Some(current_pos) = self.active_visible_pos() else {
            return false;
        };
        let Some(next_pos) = current_pos.checked_add_signed(delta) else {
            return false;
        };
        let Some(next_row) = self.visible_rows.get(next_pos).copied() else {
            return false;
        };
        if next_row == self.active_row {
            return false;
        }
        self.active_row = next_row;
        true
    }

    fn active_cell(&self) -> Option<&dyn InteractiveNode> {
        let row = self.rows.get(self.active_row)?;
        let cell = row.cells.get(self.active_col)?;
        Some(cell.as_ref())
    }

    fn active_cell_mut(&mut self) -> Option<&mut Box<dyn InteractiveNode>> {
        let row = self.rows.get_mut(self.active_row)?;
        row.cells.get_mut(self.active_col)
    }

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
        let mut completion_menus = HashMap::<String, CompletionMenu>::new();
        if let Some(cell_id) = focused_cell_id.as_deref()
            && let Some(menu) = ctx.completion_menus.get(self.base.id())
        {
            completion_menus.insert(cell_id.to_string(), menu.clone());
        }

        RenderContext {
            focused_id: focused_cell_id,
            terminal_size: ctx.terminal_size,
            visible_errors: HashMap::new(),
            invalid_hidden: HashSet::new(),
            completion_menus,
        }
    }

    fn fallback_context(&self) -> RenderContext {
        RenderContext {
            focused_id: None,
            terminal_size: TerminalSize {
                width: 80,
                height: 24,
            },
            visible_errors: HashMap::new(),
            invalid_hidden: HashSet::new(),
            completion_menus: HashMap::new(),
        }
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

    fn compute_column_widths(&self, ctx: &RenderContext) -> Vec<usize> {
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
                    width = width.max(span_line_width(line.as_slice()));
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
        if self.filter_visible {
            let filter_ctx = self.child_context(
                ctx,
                if focused && self.filter_focus {
                    Some(self.filter.id().to_string())
                } else {
                    None
                },
            );
            let mut filter_line =
                vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
            filter_line.extend(
                self.filter
                    .draw(&filter_ctx)
                    .lines
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
            );
            lines.push(filter_line);
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
            let style = if focused {
                Style::new().color(Color::Cyan).bold()
            } else if sorted {
                Style::new().color(Color::Green).bold()
            } else {
                Style::default()
            };
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
        if self.filter_visible {
            let filter_ctx = self.child_context(
                ctx,
                if focused && self.filter_focus {
                    Some(self.filter.id().to_string())
                } else {
                    None
                },
            );
            let mut filter_line =
                vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
            filter_line.extend(
                self.filter
                    .draw(&filter_ctx)
                    .lines
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
            );
            lines.push(filter_line);
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
            let style = if focused {
                Style::new().color(Color::Cyan).bold()
            } else if sorted {
                Style::new().color(Color::Green).bold()
            } else {
                Style::default()
            };
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
        lines
    }

    fn body_row_start(&self) -> u16 {
        let label_rows = if self.base.label().is_empty() { 0 } else { 1 };
        let filter_rows = if self.filter_visible { 1 } else { 0 };
        match self.style {
            TableStyle::Grid => label_rows + filter_rows + 3,
            TableStyle::Clean => label_rows + filter_rows + 1,
        }
    }

    fn body_col_starts(&self, col_widths: &[usize]) -> Vec<u16> {
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

    fn on_key_header(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char('i') => {
                if self.rows.is_empty() {
                    self.add_row();
                } else {
                    self.focus = TableFocus::Body;
                    self.active_row = self.rows.len().saturating_sub(1);
                    self.insert_row_after_active();
                }
                self.edit_mode = true;
                InteractionResult::handled()
            }
            KeyCode::Tab | KeyCode::Right => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + 1) % self.columns.len();
                InteractionResult::handled()
            }
            KeyCode::BackTab | KeyCode::Left => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + self.columns.len() - 1) % self.columns.len();
                InteractionResult::handled()
            }
            KeyCode::Down => {
                if !self.visible_rows.is_empty() {
                    self.focus = TableFocus::Body;
                    self.active_row = self.visible_rows.first().copied().unwrap_or(0);
                    self.edit_mode = false;
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_sort(self.active_col);
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_key_body(&mut self, key: KeyEvent) -> InteractionResult {
        if self.move_mode {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('m') => {
                    self.move_mode = false;
                    InteractionResult::handled()
                }
                KeyCode::Up => {
                    let moved = self.move_active_row_by(-1);
                    if moved {
                        InteractionResult::handled()
                    } else {
                        InteractionResult::ignored()
                    }
                }
                KeyCode::Down => {
                    let moved = self.move_active_row_by(1);
                    if moved {
                        InteractionResult::handled()
                    } else {
                        InteractionResult::ignored()
                    }
                }
                _ => InteractionResult::handled(),
            };
        }

        if !self.edit_mode {
            return match key.code {
                KeyCode::Char('i') => {
                    self.insert_row_after_active();
                    InteractionResult::handled()
                }
                KeyCode::Char('d') => {
                    self.delete_active_row();
                    InteractionResult::handled()
                }
                KeyCode::Char('m') => {
                    self.move_mode = self.rows.len() > 1;
                    self.sort = None;
                    self.edit_mode = false;
                    InteractionResult::handled()
                }
                KeyCode::Char('e') => {
                    self.edit_mode = true;
                    InteractionResult::handled()
                }
                KeyCode::Up => {
                    if !self.move_active_visible(-1) {
                        self.focus = TableFocus::Header;
                    }
                    InteractionResult::handled()
                }
                KeyCode::Down => {
                    let _ = self.move_active_visible(1);
                    InteractionResult::handled()
                }
                KeyCode::Tab => {
                    if self.columns.is_empty() {
                        return InteractionResult::ignored();
                    }
                    self.active_col = (self.active_col + 1) % self.columns.len();
                    InteractionResult::handled()
                }
                KeyCode::BackTab => {
                    if self.columns.is_empty() {
                        return InteractionResult::ignored();
                    }
                    self.active_col =
                        (self.active_col + self.columns.len() - 1) % self.columns.len();
                    InteractionResult::handled()
                }
                KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.columns.is_empty() {
                        return InteractionResult::ignored();
                    }
                    self.active_col =
                        (self.active_col + self.columns.len() - 1) % self.columns.len();
                    InteractionResult::handled()
                }
                KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.columns.is_empty() {
                        return InteractionResult::ignored();
                    }
                    self.active_col = (self.active_col + 1) % self.columns.len();
                    InteractionResult::handled()
                }
                _ => InteractionResult::ignored(),
            };
        }

        match key.code {
            KeyCode::Esc => {
                self.edit_mode = false;
                return InteractionResult::handled();
            }
            KeyCode::Enter => {
                self.edit_mode = false;
                return InteractionResult::handled();
            }
            KeyCode::Tab => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + 1) % self.columns.len();
                return InteractionResult::handled();
            }
            KeyCode::BackTab => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + self.columns.len() - 1) % self.columns.len();
                return InteractionResult::handled();
            }
            _ => {}
        }

        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        let result = sanitize_child_result(cell.on_key(key));
        if result.handled {
            self.apply_filter(self.active_row_id());
        }
        result
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }
        match key.code {
            KeyCode::Esc => {
                self.toggle_filter_visibility();
                InteractionResult::handled()
            }
            KeyCode::Enter | KeyCode::Down => {
                self.filter_focus = false;
                InteractionResult::handled()
            }
            _ => {
                let before = self.filter_query();
                let result = sanitize_child_result(self.filter.on_key(key));
                if self.filter_query() != before {
                    self.apply_filter(self.active_row_id());
                    return InteractionResult::handled();
                }
                result
            }
        }
    }
}

impl Component for Table {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
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

        if let Some(error) = ctx.visible_errors.get(self.base.id()) {
            lines.push(vec![
                Span::styled(
                    format!("✗ {}", error),
                    Style::new().color(Color::Red).bold(),
                )
                .no_wrap(),
            ]);
        } else if ctx.invalid_hidden.contains(self.base.id()) {
            for line in &mut lines {
                for span in line {
                    if span.style.color.is_none() {
                        span.style.color = Some(Color::Red);
                    }
                }
            }
        }

        DrawOutput { lines }
    }
}

impl Interactive for Table {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }
        if self.filter_focus {
            return self.handle_filter_key(key);
        }
        if key.modifiers == KeyModifiers::NONE
            && key.code == KeyCode::Enter
            && !self.edit_mode
            && !self.move_mode
        {
            return InteractionResult::input_done();
        }
        self.clamp_focus();
        match self.focus {
            TableFocus::Header => self.on_key_header(key),
            TableFocus::Body => self.on_key_body(key),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.filter_focus {
            let before = self.filter_query();
            let result = sanitize_child_result(self.filter.on_text_action(action));
            if self.filter_query() != before {
                self.apply_filter(self.active_row_id());
                return InteractionResult::handled();
            }
            return result;
        }
        if self.focus != TableFocus::Body || !self.edit_mode {
            return InteractionResult::ignored();
        }
        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        let result = sanitize_child_result(cell.on_text_action(action));
        if result.handled {
            self.apply_filter(self.active_row_id());
        }
        result
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if self.filter_focus {
            return self.filter.completion();
        }
        if self.focus != TableFocus::Body || !self.edit_mode {
            return None;
        }
        self.active_cell_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.filter_focus {
            let local = self.filter.cursor_pos()?;
            let row = if self.base.label().is_empty() { 0 } else { 1 };
            return Some(CursorPos {
                col: local.col.saturating_add(8),
                row,
            });
        }
        if self.focus != TableFocus::Body || !self.edit_mode {
            return None;
        }
        let local = self.active_cell().and_then(|cell| cell.cursor_pos())?;

        let col_widths = self.compute_column_widths(&self.fallback_context());
        let col_starts = self.body_col_starts(col_widths.as_slice());
        let marker_offset = if !self.show_row_numbers && self.active_col == 0 {
            2
        } else {
            0
        };
        let col = col_starts
            .get(self.active_col)
            .copied()
            .unwrap_or_default()
            .saturating_add(marker_offset)
            .saturating_add(local.col);
        let row_offset = self.active_visible_pos().unwrap_or(0) as u16;
        let row = self
            .body_row_start()
            .saturating_add(row_offset)
            .saturating_add(local.row);
        Some(CursorPos { col, row })
    }

    fn value(&self) -> Option<Value> {
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let mut map = IndexMap::<String, Value>::new();
                for (col_idx, col) in self.columns.iter().enumerate() {
                    let value = row
                        .cells
                        .get(col_idx)
                        .and_then(|cell| cell.value())
                        .unwrap_or(Value::None);
                    map.insert(col.key.clone(), value);
                }
                Value::Object(map)
            })
            .collect::<Vec<_>>();
        Some(Value::List(rows))
    }

    fn set_value(&mut self, value: Value) {
        self.rows.clear();
        match value {
            Value::None => {}
            Value::List(list) => {
                for entry in list {
                    let row_id = self.next_row_id;
                    self.next_row_id = self.next_row_id.saturating_add(1);
                    self.rows.push(self.build_row(row_id, Some(&entry)));
                }
            }
            other => {
                let row_id = self.next_row_id;
                self.next_row_id = self.next_row_id.saturating_add(1);
                self.rows.push(self.build_row(row_id, Some(&other)));
            }
        }

        self.clamp_focus();
        self.apply_sort_preserving_focus(self.active_row_id());
        if self.rows.is_empty() {
            self.focus = TableFocus::Header;
        } else if self.focus == TableFocus::Header {
            self.focus = TableFocus::Body;
            self.active_row = 0;
        }
        self.move_mode = false;
        self.edit_mode = false;
        self.apply_filter(self.active_row_id());
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                if let Err(error) = cell.validate(mode) {
                    let header = self
                        .columns
                        .get(col_idx)
                        .map(|col| col.header.as_str())
                        .unwrap_or("column");
                    return Err(format!("row {}, {}: {}", row_idx + 1, header, error));
                }
            }
        }
        Ok(())
    }
}

fn sanitize_child_result(mut result: InteractionResult) -> InteractionResult {
    result
        .actions
        .retain(|action| !matches!(action, WidgetAction::InputDone));
    if result.handled {
        result.request_render = true;
    }
    result
}

fn normalize_key(header: &str) -> String {
    let mut key = String::new();
    let mut previous_underscore = false;
    for ch in header.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
            previous_underscore = false;
            continue;
        }
        if !previous_underscore && !key.is_empty() {
            key.push('_');
            previous_underscore = true;
        }
    }
    while key.ends_with('_') {
        key.pop();
    }
    if key.is_empty() {
        "col".to_string()
    } else {
        key
    }
}

fn seed_value(seed: Option<&Value>, col_idx: usize, key: &str, header: &str) -> Option<Value> {
    let seed = seed?;
    match seed {
        Value::Object(map) => map.get(key).cloned().or_else(|| map.get(header).cloned()),
        Value::List(items) => items.get(col_idx).cloned(),
        Value::None => None,
        scalar if col_idx == 0 => Some(scalar.clone()),
        _ => None,
    }
}

fn compare_cell_values(left: Option<Value>, right: Option<Value>) -> Ordering {
    match (left, right) {
        (Some(Value::Number(a)), Some(Value::Number(b))) => {
            a.partial_cmp(&b).unwrap_or(Ordering::Equal)
        }
        (Some(Value::Bool(a)), Some(Value::Bool(b))) => a.cmp(&b),
        (Some(a), Some(b)) => value_sort_text(&a).cmp(&value_sort_text(&b)),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn value_sort_text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.to_lowercase(),
        Value::Number(number) => format!("{number:020.6}"),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::None => String::new(),
        Value::List(_) | Value::Object(_) => value.to_json().to_lowercase(),
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

fn span_line_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.text.as_str()))
        .sum()
}

fn fit_spans_to_width(spans: SpanLine, width: usize) -> SpanLine {
    if width == 0 {
        return vec![];
    }

    let mut out = Vec::<Span>::new();
    let mut used = 0usize;
    for span in spans {
        if used >= width {
            break;
        }
        let available = width.saturating_sub(used);
        let clipped = clip_text_to_width(span.text.as_str(), available);
        if clipped.is_empty() {
            continue;
        }
        used = used.saturating_add(UnicodeWidthStr::width(clipped.as_str()));
        out.push(Span::styled(clipped, span.style).no_wrap());
    }

    if used < width {
        out.push(Span::new(" ".repeat(width - used)).no_wrap());
    }
    out
}

fn clip_text_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut used = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(ch_width) > max_width {
            break;
        }
        out.push(ch);
        used = used.saturating_add(ch_width);
    }
    out
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
        line.extend(fit_spans_to_width(
            cells.get(idx).cloned().unwrap_or_default(),
            *width,
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
        line.extend(fit_spans_to_width(
            cells.get(idx).cloned().unwrap_or_default(),
            *width,
        ));
    }
    line
}
