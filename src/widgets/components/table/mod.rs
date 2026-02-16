use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::core::value::Value;
use crate::runtime::event::WidgetAction;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers, TerminalSize};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
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
    AddRecord,
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
    columns: Vec<ColumnDef>,
    rows: Vec<RowState>,
    focus: TableFocus,
    active_row: usize,
    active_col: usize,
    sort: Option<(usize, SortDirection)>,
    next_row_id: u64,
}

impl Table {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            style: TableStyle::Grid,
            columns: Vec::new(),
            rows: Vec::new(),
            focus: TableFocus::AddRecord,
            active_row: 0,
            active_col: 0,
            sort: None,
            next_row_id: 1,
        }
    }

    pub fn with_style(mut self, style: TableStyle) -> Self {
        self.style = style;
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
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::AddRecord;
            }
            return;
        }

        self.active_row = self.active_row.min(self.rows.len().saturating_sub(1));
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
        self.row_digits().saturating_add(1)
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
                for row_idx in 0..self.rows.len() {
                    let line = self.render_cell_line(row_idx, col_idx, ctx, false);
                    width = width.max(span_line_width(line.as_slice()));
                }
                width
            })
            .collect()
    }

    fn render_grid(&self, ctx: &RenderContext, col_widths: &[usize]) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        let mut widths = Vec::<usize>::with_capacity(col_widths.len().saturating_add(1));
        widths.push(self.row_index_width());
        widths.extend_from_slice(col_widths);

        lines.push(grid_border_line('┌', '┬', '┐', widths.as_slice()));

        let mut header_cells = Vec::<SpanLine>::with_capacity(widths.len());
        header_cells.push(vec![Span::new("#").no_wrap()]);
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
            header_cells.push(vec![
                Span::styled(self.header_text(col_idx), style).no_wrap(),
            ]);
        }
        lines.push(grid_row(header_cells, widths.as_slice()));
        lines.push(grid_border_line('├', '┼', '┤', widths.as_slice()));

        for row_idx in 0..self.rows.len() {
            let marker = if self.focus == TableFocus::Body && self.active_row == row_idx {
                '❯'
            } else {
                ' '
            };
            let idx_text = format!("{marker}{:>w$}", row_idx + 1, w = self.row_digits());

            let mut row_cells = Vec::<SpanLine>::with_capacity(widths.len());
            row_cells.push(vec![Span::new(idx_text).no_wrap()]);
            for (col_idx, _) in self.columns.iter().enumerate() {
                let focused = self.focus == TableFocus::Body
                    && self.active_row == row_idx
                    && self.active_col == col_idx;
                row_cells.push(self.render_cell_line(row_idx, col_idx, ctx, focused));
            }
            lines.push(grid_row(row_cells, widths.as_slice()));
        }

        lines.push(grid_border_line('├', '┴', '┤', widths.as_slice()));
        lines.push(grid_add_record_line(
            widths.as_slice(),
            self.focus == TableFocus::AddRecord,
        ));
        lines.push(full_grid_border_line('└', '┘', widths.as_slice()));
        lines
    }

    fn render_clean(&self, ctx: &RenderContext, col_widths: &[usize]) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        let mut header_cells = Vec::<SpanLine>::new();
        header_cells.push(vec![Span::new("#").no_wrap()]);
        let mut clean_widths = Vec::<usize>::new();
        clean_widths.push(self.row_index_width());
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
            header_cells.push(vec![
                Span::styled(self.header_text(col_idx), style).no_wrap(),
            ]);
        }
        lines.push(clean_row(header_cells, clean_widths.as_slice()));

        for row_idx in 0..self.rows.len() {
            let marker = if self.focus == TableFocus::Body && self.active_row == row_idx {
                '❯'
            } else {
                ' '
            };
            let idx_text = format!("{marker}{:>w$}", row_idx + 1, w = self.row_digits());

            let mut row_cells = Vec::<SpanLine>::new();
            row_cells.push(vec![Span::new(idx_text).no_wrap()]);
            for (col_idx, _) in self.columns.iter().enumerate() {
                let focused = self.focus == TableFocus::Body
                    && self.active_row == row_idx
                    && self.active_col == col_idx;
                row_cells.push(self.render_cell_line(row_idx, col_idx, ctx, focused));
            }
            lines.push(clean_row(row_cells, clean_widths.as_slice()));
        }

        let marker = if self.focus == TableFocus::AddRecord {
            "❯"
        } else {
            " "
        };
        lines.push(vec![
            Span::styled(
                format!("{marker} + Add record"),
                Style::new().color(Color::Green).bold(),
            )
            .no_wrap(),
        ]);
        lines
    }

    fn body_row_start(&self) -> u16 {
        let label_rows = if self.base.label().is_empty() { 0 } else { 1 };
        match self.style {
            TableStyle::Grid => label_rows + 3,
            TableStyle::Clean => label_rows + 1,
        }
    }

    fn body_col_starts(&self, col_widths: &[usize]) -> Vec<u16> {
        match self.style {
            TableStyle::Grid => {
                let mut widths = Vec::<usize>::with_capacity(col_widths.len().saturating_add(1));
                widths.push(self.row_index_width());
                widths.extend_from_slice(col_widths);

                let mut starts = Vec::<u16>::with_capacity(widths.len());
                let mut cursor = 2u16;
                for width in &widths {
                    starts.push(cursor);
                    cursor = cursor.saturating_add((*width as u16).saturating_add(3));
                }
                starts.into_iter().skip(1).collect()
            }
            TableStyle::Clean => {
                let mut starts = Vec::<u16>::with_capacity(col_widths.len());
                let mut cursor = (self.row_index_width() as u16).saturating_add(2);
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
                if self.rows.is_empty() {
                    self.focus = TableFocus::AddRecord;
                } else {
                    self.focus = TableFocus::Body;
                    self.active_row = 0;
                }
                InteractionResult::handled()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_sort(self.active_col);
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_key_body(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Up => {
                if self.active_row == 0 {
                    self.focus = TableFocus::Header;
                } else {
                    self.active_row = self.active_row.saturating_sub(1);
                }
                return InteractionResult::handled();
            }
            KeyCode::Down => {
                if self.active_row + 1 >= self.rows.len() {
                    self.focus = TableFocus::AddRecord;
                } else {
                    self.active_row += 1;
                }
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
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + self.columns.len() - 1) % self.columns.len();
                return InteractionResult::handled();
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.columns.is_empty() {
                    return InteractionResult::ignored();
                }
                self.active_col = (self.active_col + 1) % self.columns.len();
                return InteractionResult::handled();
            }
            _ => {}
        }

        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        sanitize_child_result(cell.on_key(key))
    }

    fn on_key_add_record(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('+') => {
                self.add_row();
                InteractionResult::handled()
            }
            KeyCode::Up => {
                if self.rows.is_empty() {
                    self.focus = TableFocus::Header;
                } else {
                    self.focus = TableFocus::Body;
                    self.active_row = self.rows.len().saturating_sub(1);
                }
                InteractionResult::handled()
            }
            KeyCode::Tab | KeyCode::Right => {
                self.focus = TableFocus::Header;
                InteractionResult::handled()
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.focus = TableFocus::Header;
                if !self.columns.is_empty() {
                    self.active_col =
                        (self.active_col + self.columns.len() - 1) % self.columns.len();
                }
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
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
        let col_widths = self.compute_column_widths(ctx);
        let mut lines = match self.style {
            TableStyle::Grid => self.render_grid(ctx, col_widths.as_slice()),
            TableStyle::Clean => self.render_clean(ctx, col_widths.as_slice()),
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
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
            self.add_row();
            return InteractionResult::handled();
        }

        self.clamp_focus();
        match self.focus {
            TableFocus::Header => self.on_key_header(key),
            TableFocus::Body => self.on_key_body(key),
            TableFocus::AddRecord => self.on_key_add_record(key),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.focus != TableFocus::Body {
            return InteractionResult::ignored();
        }
        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        sanitize_child_result(cell.on_text_action(action))
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if self.focus != TableFocus::Body {
            return None;
        }
        self.active_cell_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.focus != TableFocus::Body {
            return None;
        }
        let local = self.active_cell().and_then(|cell| cell.cursor_pos())?;

        let col_widths = self.compute_column_widths(&self.fallback_context());
        let col_starts = self.body_col_starts(col_widths.as_slice());
        let col = col_starts
            .get(self.active_col)
            .copied()
            .unwrap_or_default()
            .saturating_add(local.col);
        let row = self
            .body_row_start()
            .saturating_add(self.active_row as u16)
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
            self.focus = TableFocus::AddRecord;
        } else if self.focus == TableFocus::AddRecord {
            self.focus = TableFocus::Body;
            self.active_row = 0;
        }
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

fn full_grid_border_line(left: char, right: char, widths: &[usize]) -> SpanLine {
    let border_style = Style::new().color(Color::DarkGrey);
    let inner = grid_inner_width(widths);
    vec![
        Span::styled(left.to_string(), border_style).no_wrap(),
        Span::styled("─".repeat(inner), border_style).no_wrap(),
        Span::styled(right.to_string(), border_style).no_wrap(),
    ]
}

fn grid_inner_width(widths: &[usize]) -> usize {
    widths.iter().map(|w| w + 2).sum::<usize>() + widths.len().saturating_sub(1)
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

fn grid_add_record_line(widths: &[usize], focused: bool) -> SpanLine {
    let border_style = Style::new().color(Color::DarkGrey);
    let marker = if focused { "❯" } else { " " };
    let content = format!("{marker} + Add record");
    let inner = grid_inner_width(widths);
    let text = clip_text_to_width(content.as_str(), inner);
    let used = UnicodeWidthStr::width(text.as_str());
    vec![
        Span::styled("│", border_style).no_wrap(),
        Span::styled(text, Style::new().color(Color::Green).bold()).no_wrap(),
        Span::new(" ".repeat(inner.saturating_sub(used))).no_wrap(),
        Span::styled("│", border_style).no_wrap(),
    ]
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
