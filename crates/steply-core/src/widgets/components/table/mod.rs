use std::cmp::Ordering;
use std::sync::Arc;

use indexmap::IndexMap;
use unicode_width::UnicodeWidthStr;

use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::filter as filter_utils;
use crate::widgets::shared::list_policy;
use crate::widgets::shared::validation::decorate_component_validation;
use crate::widgets::shared::value_seed::{normalize_ascii_key, seed_value_from_record};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, InteractiveNode, RenderContext, TextAction, ValidationMode,
};

mod interaction;
mod render;

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
enum TableBodyMode {
    Navigate,
    Edit,
    Move,
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
    body_mode: TableBodyMode,
    filter: filter_utils::ListFilter,
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
            body_mode: TableBodyMode::Navigate,
            filter: filter_utils::ListFilter::new(
                format!("{id}__filter"),
                filter_utils::FilterEscBehavior::Hide,
                true,
            ),
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
        if !self.rows.is_empty() {
            self.focus = TableFocus::Body;
            self.active_row = 0;
            self.active_col = list_policy::clamp_index(0, self.columns.len());
            self.body_mode = TableBodyMode::Edit;
            self.apply_filter(self.active_row_id());
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
        self.active_col = list_policy::clamp_index(self.active_col, self.columns.len());
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
        self.active_col = list_policy::clamp_index(self.active_col, self.columns.len());
        self.body_mode = TableBodyMode::Edit;
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
        self.body_mode = TableBodyMode::Navigate;
        if self.rows.is_empty() {
            self.focus = TableFocus::Header;
            self.active_row = 0;
        } else {
            self.active_row = list_policy::clamp_index(self.active_row, self.rows.len());
        }
        self.apply_filter(preferred);
    }

    fn move_active_row_by(&mut self, delta: isize) -> bool {
        if self.rows.len() < 2 || self.focus != TableFocus::Body {
            return false;
        }
        let Some(next_idx) = list_policy::move_by(self.active_row, delta, self.rows.len()) else {
            return false;
        };
        self.sort = None;
        let active_id = self.rows.get(self.active_row).map(|row| row.id);
        self.rows.swap(self.active_row, next_idx);
        self.active_row = next_idx;
        self.apply_filter(active_id);
        true
    }

    fn push_column(&mut self, header: String, make_cell: CellFactory) {
        let key = self.unique_column_key(header.as_str());
        let min_width = UnicodeWidthStr::width(header.as_str()).max(10);
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
        let base = normalize_ascii_key(header, "col");
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
            if let Some(value) =
                seed_value_from_record(seed, col_idx, col.key.as_str(), col.header.as_str())
            {
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
        self.active_col = list_policy::clamp_index(self.active_col, self.columns.len());

        if self.rows.is_empty() {
            self.active_row = 0;
            self.body_mode = TableBodyMode::Navigate;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        self.active_row = list_policy::clamp_index(self.active_row, self.rows.len());
    }

    fn filter_query(&self) -> String {
        self.filter.query()
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
            self.visible_rows = (0..self.rows.len())
                .filter(|&row_idx| {
                    (0..self.columns.len()).any(|col_idx| {
                        let text = self.cell_filter_text(row_idx, col_idx);
                        list_policy::text_matches(query, text.as_str())
                    })
                })
                .collect();
        }

        if self.rows.is_empty() {
            self.active_row = 0;
            self.body_mode = TableBodyMode::Navigate;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        if self.visible_rows.is_empty() {
            self.body_mode = TableBodyMode::Navigate;
            if self.focus == TableFocus::Body {
                self.focus = TableFocus::Header;
            }
            return;
        }

        let preferred_row_idx =
            preferred_row_id.and_then(|id| self.rows.iter().position(|row| row.id == id));
        if let Some(preferred) = preferred_row_idx.filter(|idx| self.visible_rows.contains(idx)) {
            self.active_row = preferred;
        } else if !self.visible_rows.contains(&self.active_row)
            && let Some(next) = self.visible_rows.first().copied()
        {
            self.active_row = next;
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
        let Some(next_pos) = list_policy::move_by(current_pos, delta, self.visible_rows.len())
        else {
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

    fn is_body_edit_mode(&self) -> bool {
        self.focus == TableFocus::Body && self.body_mode == TableBodyMode::Edit
    }

    fn is_body_move_mode(&self) -> bool {
        self.focus == TableFocus::Body && self.body_mode == TableBodyMode::Move
    }

    fn set_body_mode(&mut self, mode: TableBodyMode) {
        self.body_mode = mode;
    }
}

impl LeafComponent for Table {}

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
