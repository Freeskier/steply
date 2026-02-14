use std::collections::BTreeMap;

use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};

// ── Flat node representation ──────────────────────────────────────────────────

/// A single row in the editor — one key/value pair at a given nesting depth.
#[derive(Debug, Clone)]
struct ObjNode {
    key: String,
    value: Value,
    depth: usize,
    /// For List children the key is the index stringified ("0", "1", …).
    is_index: bool,
}

// ── Editor modes ──────────────────────────────────────────────────────────────

enum Mode {
    /// Normal navigation.
    Normal,
    /// Editing the value of node at `row` (TextInput).
    EditValue { row: usize },
    /// Editing the key of node at `row` (TextInput).
    EditKey { row: usize },
    /// Inserting a new node after `after_row` — first choose type with SelectInput.
    InsertType {
        after_row: usize,
        key_input: TextInput,
        type_select: SelectInput,
    },
    /// After type chosen, editing the initial value (only for scalar types).
    InsertValue {
        after_row: usize,
        key: String,
        value_type: InsertValueType,
        value_input: TextInput,
    },
    /// Confirm delete of node at `row` (ChoiceInput). Subtree shown in red.
    ConfirmDelete { row: usize, choice: ChoiceInput },
    /// Move mode — node at `row` is highlighted yellow, arrows move it.
    Move { row: usize },
}

#[derive(Debug, Clone, Copy)]
enum InsertValueType {
    Text,
    Number,
    Object,
    Array,
}

// ── ObjectEditor ─────────────────────────────────────────────────────────────

pub struct ObjectEditor {
    base: WidgetBase,
    /// The canonical value being edited.
    value: Value,
    /// Flat list of visible rows (rebuilt on every structural change).
    rows: Vec<ObjNode>,
    /// Which rows are expanded (by their path string).
    expanded: std::collections::HashSet<String>,
    active: usize,
    scroll: ScrollState,
    mode: Mode,
    /// TextInput reused for value editing.
    edit_input: TextInput,
    submit_target: Option<String>,
}

impl ObjectEditor {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let edit_id = format!("{id}__edit");
        let mut this = Self {
            base: WidgetBase::new(id, label),
            value: Value::Object(BTreeMap::new()),
            rows: Vec::new(),
            expanded: std::collections::HashSet::new(),
            active: 0,
            scroll: ScrollState::new(Some(12)),
            mode: Mode::Normal,
            edit_input: TextInput::new(edit_id, ""),
            submit_target: None,
        };
        this.rebuild();
        this
    }

    pub fn with_value(mut self, value: Value) -> Self {
        self.value = value;
        // Expand all top-level nodes by default
        self.expand_all_top_level();
        self.rebuild();
        self
    }

    pub fn with_max_visible(mut self, n: usize) -> Self {
        self.scroll.max_visible = Some(n);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    // ── Path helpers ─────────────────────────────────────────────────────────

    /// Build a dot-separated path string for a row (used as expansion key).
    fn row_path(rows: &[ObjNode], idx: usize) -> String {
        // Collect the chain of keys from root to this node.
        let mut path_parts: Vec<String> = Vec::new();
        let mut target_depth = rows[idx].depth;
        path_parts.push(rows[idx].key.clone());
        if target_depth == 0 {
            return rows[idx].key.clone();
        }
        for i in (0..idx).rev() {
            if rows[i].depth < target_depth {
                target_depth = rows[i].depth;
                path_parts.push(rows[i].key.clone());
                if target_depth == 0 {
                    break;
                }
            }
        }
        path_parts.reverse();
        path_parts.join(".")
    }

    fn expand_all_top_level(&mut self) {
        // We'll rebuild later; for now just mark top-level Object/List keys as expanded.
        match &self.value {
            Value::Object(map) => {
                for key in map.keys() {
                    self.expanded.insert(key.clone());
                }
            }
            Value::List(arr) => {
                for i in 0..arr.len() {
                    self.expanded.insert(i.to_string());
                }
            }
            _ => {}
        }
    }

    // ── Flat list builder ────────────────────────────────────────────────────

    fn rebuild(&mut self) {
        self.rows.clear();
        Self::flatten(&self.value, &self.expanded, &mut self.rows, 0, "");
        ScrollState::clamp_active(&mut self.active, self.rows.len());
        self.scroll.ensure_visible(self.active, self.rows.len());
    }

    fn flatten(
        value: &Value,
        expanded: &std::collections::HashSet<String>,
        rows: &mut Vec<ObjNode>,
        depth: usize,
        prefix: &str,
    ) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    let is_container = matches!(child, Value::Object(_) | Value::List(_));
                    rows.push(ObjNode {
                        key: key.clone(),
                        value: child.clone(),
                        depth,
                        is_index: false,
                    });
                    if is_container && expanded.contains(&path) {
                        Self::flatten(child, expanded, rows, depth + 1, &path);
                    }
                }
            }
            Value::List(arr) => {
                for (i, child) in arr.iter().enumerate() {
                    let key = i.to_string();
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    let is_container = matches!(child, Value::Object(_) | Value::List(_));
                    rows.push(ObjNode {
                        key: key.clone(),
                        value: child.clone(),
                        depth,
                        is_index: true,
                    });
                    if is_container && expanded.contains(&path) {
                        Self::flatten(child, expanded, rows, depth + 1, &path);
                    }
                }
            }
            _ => {}
        }
    }

    // ── Value mutation helpers ────────────────────────────────────────────────

    /// Get a mutable reference to the value at a given dot-path.
    fn value_at_path_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        if path.is_empty() {
            return Some(root);
        }
        let mut current = root;
        for part in path.split('.') {
            match current {
                Value::Object(map) => {
                    current = map.get_mut(part)?;
                }
                Value::List(arr) => {
                    let idx: usize = part.parse().ok()?;
                    current = arr.get_mut(idx)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Get a parent path and key for a row.
    fn parent_path_and_key(rows: &[ObjNode], idx: usize) -> (String, String) {
        let key = rows[idx].key.clone();
        let depth = rows[idx].depth;
        if depth == 0 {
            return (String::new(), key);
        }
        // Find parent: first node going backwards with depth - 1
        for i in (0..idx).rev() {
            if rows[i].depth == depth - 1 {
                let parent_path = Self::row_path(rows, i);
                return (parent_path, key);
            }
        }
        (String::new(), key)
    }

    /// Parse a string into a Value using autodetect.
    /// `"quoted"` → String (forced), `123` → Number, `true`/`false` → Bool,
    /// `null` → None, everything else → String.
    pub fn parse_scalar(s: &str) -> Value {
        let s = s.trim();
        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
            return Value::Text(s[1..s.len() - 1].to_string());
        }
        if s == "null" {
            return Value::None;
        }
        if s == "true" {
            return Value::Bool(true);
        }
        if s == "false" {
            return Value::Bool(false);
        }
        if let Ok(n) = s.parse::<f64>() {
            return Value::Number(n);
        }
        Value::Text(s.to_string())
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let len = self.rows.len() as isize;
        self.active = ((self.active as isize + delta + len) % len) as usize;
        self.scroll.ensure_visible(self.active, self.rows.len());
    }

    fn toggle_expand(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let path = Self::row_path(&self.rows, self.active);
        let is_container = matches!(
            self.rows[self.active].value,
            Value::Object(_) | Value::List(_)
        );
        if !is_container {
            return;
        }
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        self.rebuild();
    }

    // ── Edit value ────────────────────────────────────────────────────────────

    fn start_edit_value(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let row = self.active;
        let current = match &self.rows[row].value {
            Value::Object(_) | Value::List(_) => return, // can't edit containers inline
            v => v.to_text_scalar().unwrap_or_else(|| "null".to_string()),
        };
        self.edit_input.set_value(Value::Text(current));
        self.mode = Mode::EditValue { row };
    }

    fn start_edit_key(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let row = self.active;
        if self.rows[row].is_index {
            return; // can't rename array indices
        }
        let key = self.rows[row].key.clone();
        self.edit_input.set_value(Value::Text(key));
        self.mode = Mode::EditKey { row };
    }

    fn commit_edit_value(&mut self) {
        let Mode::EditValue { row } = self.mode else {
            return;
        };
        let text = self
            .edit_input
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default();
        let new_val = Self::parse_scalar(&text);
        let (parent_path, key) = Self::parent_path_and_key(&self.rows, row);
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &parent_path) {
            match parent {
                Value::Object(map) => {
                    map.insert(key, new_val);
                }
                Value::List(arr) => {
                    if let Ok(idx) = key.parse::<usize>() {
                        if idx < arr.len() {
                            arr[idx] = new_val;
                        }
                    }
                }
                _ => {}
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    fn commit_edit_key(&mut self) {
        let Mode::EditKey { row } = self.mode else {
            return;
        };
        let new_key = self
            .edit_input
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default();
        if new_key.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        let (parent_path, old_key) = Self::parent_path_and_key(&self.rows, row);
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &parent_path) {
            if let Value::Object(map) = parent {
                if let Some(val) = map.remove(&old_key) {
                    map.insert(new_key, val);
                }
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    // ── Insert ────────────────────────────────────────────────────────────────

    fn start_insert(&mut self) {
        let after_row = self.active;
        let key_input = TextInput::new(format!("{}_insert_key", self.base.id()), "");
        let type_select = SelectInput::new(
            format!("{}_insert_type", self.base.id()),
            "Type",
            vec![
                "text".into(),
                "number".into(),
                "object".into(),
                "array".into(),
            ],
        );
        self.mode = Mode::InsertType {
            after_row,
            key_input,
            type_select,
        };
    }

    fn commit_insert_type(&mut self) {
        let Mode::InsertType {
            after_row,
            ref key_input,
            ref type_select,
        } = self.mode
        else {
            return;
        };
        let key = key_input
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default();
        if key.is_empty() {
            self.mode = Mode::Normal;
            return;
        }

        let type_val = type_select
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_else(|| "text".into());

        let after_row = after_row;
        let key_clone = key.clone();

        let value_type = match type_val.as_str() {
            "number" => InsertValueType::Number,
            "object" => InsertValueType::Object,
            "array" => InsertValueType::Array,
            _ => InsertValueType::Text,
        };

        match value_type {
            InsertValueType::Object | InsertValueType::Array => {
                // Insert immediately with empty container
                let new_val = if matches!(value_type, InsertValueType::Object) {
                    Value::Object(BTreeMap::new())
                } else {
                    Value::List(Vec::new())
                };
                self.insert_after(after_row, key_clone, new_val);
                self.mode = Mode::Normal;
                self.rebuild();
            }
            _ => {
                let mut value_input = TextInput::new(format!("{}_insert_val", self.base.id()), "");
                value_input.set_value(Value::Text(String::new()));
                self.mode = Mode::InsertValue {
                    after_row,
                    key: key_clone,
                    value_type,
                    value_input,
                };
            }
        }
    }

    fn commit_insert_value(&mut self) {
        let Mode::InsertValue {
            after_row,
            ref key,
            value_type,
            ref value_input,
        } = self.mode
        else {
            return;
        };
        let text = value_input
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default();
        let new_val = match value_type {
            InsertValueType::Number => Value::Number(text.parse::<f64>().unwrap_or(0.0)),
            _ => Self::parse_scalar(&text),
        };
        let after_row = after_row;
        let key = key.clone();
        self.insert_after(after_row, key, new_val);
        self.mode = Mode::Normal;
        self.rebuild();
    }

    fn insert_after(&mut self, after_row: usize, new_key: String, new_val: Value) {
        if self.rows.is_empty() {
            // Insert into root
            match &mut self.value {
                Value::Object(map) => {
                    map.insert(new_key, new_val);
                }
                Value::List(arr) => {
                    arr.push(new_val);
                }
                _ => {}
            }
            return;
        }
        // Find parent of after_row
        let (parent_path, sibling_key) = Self::parent_path_and_key(&self.rows, after_row);
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &parent_path) {
            match parent {
                Value::Object(map) => {
                    // BTreeMap is sorted; insert at end of parent (can't insert at specific pos)
                    map.insert(new_key, new_val);
                }
                Value::List(arr) => {
                    let idx: usize = sibling_key.parse().unwrap_or(arr.len());
                    arr.insert(idx + 1, new_val);
                }
                _ => {}
            }
        }
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    fn start_delete(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let row = self.active;
        let choice = ChoiceInput::new(
            format!("{}_confirm_del", self.base.id()),
            format!("Delete {}?", self.rows[row].key),
            vec!["No".into(), "Yes".into()],
        )
        .with_bullets(false);
        self.mode = Mode::ConfirmDelete { row, choice };
    }

    fn commit_delete(&mut self, confirmed: bool) {
        let Mode::ConfirmDelete { row, .. } = self.mode else {
            return;
        };
        if confirmed {
            let (parent_path, key) = Self::parent_path_and_key(&self.rows, row);
            if let Some(parent) = Self::value_at_path_mut(&mut self.value, &parent_path) {
                match parent {
                    Value::Object(map) => {
                        map.remove(&key);
                    }
                    Value::List(arr) => {
                        if let Ok(idx) = key.parse::<usize>() {
                            if idx < arr.len() {
                                arr.remove(idx);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if self.active > 0 {
                self.active -= 1;
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    // ── Move ─────────────────────────────────────────────────────────────────

    fn start_move(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.mode = Mode::Move { row: self.active };
    }

    fn move_node(&mut self, delta: isize) {
        let current = match &self.mode {
            Mode::Move { row } => *row,
            _ => return,
        };
        let len = self.rows.len() as isize;
        if len <= 1 {
            return;
        }

        let target = ((current as isize + delta + len) % len) as usize;
        let (cur_parent, cur_key) = Self::parent_path_and_key(&self.rows, current);
        let target_key = self.rows[target].key.clone();

        if cur_parent == Self::parent_path_and_key(&self.rows, target).0 {
            if let Some(parent) = Self::value_at_path_mut(&mut self.value, &cur_parent) {
                match parent {
                    Value::List(arr) => {
                        let ci: usize = cur_key.parse().unwrap_or(0);
                        let ti: usize = target_key.parse().unwrap_or(0);
                        if ci < arr.len() && ti < arr.len() {
                            arr.swap(ci, ti);
                        }
                    }
                    Value::Object(map) => {
                        let ci = map.keys().position(|k| k == &cur_key);
                        let ti = map.keys().position(|k| k == &target_key);
                        if let (Some(ci), Some(ti)) = (ci, ti) {
                            let mut entries: Vec<(String, Value)> =
                                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                            entries.swap(ci, ti);
                            *map = entries.into_iter().collect();
                        }
                    }
                    _ => {}
                }
            }
        } else {
            let val = {
                let (p, k) = (cur_parent.clone(), cur_key.clone());
                if let Some(parent) = Self::value_at_path_mut(&mut self.value, &p) {
                    match parent {
                        Value::Object(map) => map.remove(&k),
                        Value::List(arr) => {
                            if let Ok(i) = k.parse::<usize>() {
                                if i < arr.len() {
                                    Some(arr.remove(i))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            };
            if let Some(val) = val {
                let (p, k) = Self::parent_path_and_key(&self.rows, target);
                if let Some(parent) = Self::value_at_path_mut(&mut self.value, &p) {
                    match parent {
                        Value::Object(map) => {
                            map.insert(cur_key.clone(), val);
                        }
                        Value::List(arr) => {
                            if let Ok(i) = k.parse::<usize>() {
                                arr.insert(i, val);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        self.rebuild();
        let new_pos = self
            .rows
            .iter()
            .position(|r| r.key == cur_key)
            .unwrap_or(target);
        self.active = new_pos;
        self.scroll.ensure_visible(self.active, self.rows.len());
        self.mode = Mode::Move { row: new_pos };
    }

    // ── Rendering ────────────────────────────────────────────────────────────

    fn value_display(val: &Value) -> (String, Style) {
        match val {
            Value::Text(s) => (s.clone(), Style::new().color(Color::Green)),
            Value::Number(n) => {
                let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                };
                (s, Style::new().color(Color::Cyan))
            }
            Value::Bool(b) => (b.to_string(), Style::new().color(Color::Yellow)),
            Value::None => ("null".to_string(), Style::new().color(Color::DarkGrey)),
            Value::Object(m) => (
                format!("{{{}}}", m.len()),
                Style::new().color(Color::DarkGrey),
            ),
            Value::List(a) => (
                format!("[{}]", a.len()),
                Style::new().color(Color::DarkGrey),
            ),
        }
    }

    fn is_expanded(&self, row: usize) -> bool {
        let path = Self::row_path(&self.rows, row);
        self.expanded.contains(&path)
    }

    fn subtree_rows(&self, row: usize) -> std::ops::Range<usize> {
        let depth = self.rows[row].depth;
        let start = row + 1;
        let end = self.rows[start..]
            .iter()
            .position(|r| r.depth <= depth)
            .map(|p| start + p)
            .unwrap_or(self.rows.len());
        start..end
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

impl Component for ObjectEditor {
    fn children(&self) -> &[Node] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

// ── Drawable ─────────────────────────────────────────────────────────────────

impl Drawable for ObjectEditor {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);

        let inactive = Style::new().color(Color::DarkGrey);
        let cursor_st = Style::new().color(Color::Yellow);
        let key_st = Style::new().color(Color::White).bold();
        let key_dim = Style::new().color(Color::DarkGrey);
        let red_st = Style::new().color(Color::Red);
        let yellow_st = Style::new().color(Color::Yellow);

        let total = self.rows.len();
        let (start, end) = self.scroll.visible_range(total);

        // Determine which rows are "red" (delete confirm subtree) or "yellow" (move)
        let red_range: Option<std::ops::Range<usize>> = match &self.mode {
            Mode::ConfirmDelete { row, .. } => {
                let r = *row;
                Some(r..self.subtree_rows(r).end)
            }
            _ => None,
        };
        let yellow_row: Option<usize> = match &self.mode {
            Mode::Move { row } => Some(*row),
            _ => None,
        };

        let mut lines: Vec<Vec<Span>> = Vec::new();

        // Label
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        for vis in start..end {
            let row = &self.rows[vis];
            let is_active = focused && vis == self.active;
            let in_red = red_range
                .as_ref()
                .map(|r| r.contains(&vis))
                .unwrap_or(false);
            let in_yellow = yellow_row == Some(vis);

            let cursor = if is_active { "❯" } else { " " };
            let cursor_span =
                Span::styled(cursor, if is_active { cursor_st } else { inactive }).no_wrap();

            let indent = "  ".repeat(row.depth);
            let indent_span = Span::styled(format!(" {indent}"), inactive).no_wrap();

            // Expand icon for containers
            let is_container = matches!(row.value, Value::Object(_) | Value::List(_));
            let icon = if is_container {
                if self.is_expanded(vis) {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let icon_span = Span::styled(icon, inactive).no_wrap();

            let key_style = if in_red {
                red_st
            } else if in_yellow {
                yellow_st
            } else if row.is_index {
                key_dim
            } else {
                key_st
            };

            // In edit-key mode for this row — show TextInput instead
            let key_spans: Vec<Span> = if let Mode::EditKey { row: er } = &self.mode {
                if *er == vis {
                    let val = self
                        .edit_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    vec![
                        Span::styled(format!("[ {val}_ ]"), Style::new().color(Color::Cyan))
                            .no_wrap(),
                    ]
                } else {
                    vec![Span::styled(format!("{}:", row.key), key_style).no_wrap()]
                }
            } else {
                vec![Span::styled(format!("{}:", row.key), key_style).no_wrap()]
            };

            // Value display
            let value_spans: Vec<Span> = if let Mode::EditValue { row: er } = &self.mode {
                if *er == vis {
                    let val = self
                        .edit_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    vec![
                        Span::new(" ").no_wrap(),
                        Span::styled(format!("[ {val}_ ]"), Style::new().color(Color::Cyan))
                            .no_wrap(),
                    ]
                } else {
                    let (text, style) = Self::value_display(&row.value);
                    let style = if in_red {
                        red_st
                    } else if in_yellow {
                        yellow_st
                    } else {
                        style
                    };
                    vec![
                        Span::new(" ").no_wrap(),
                        Span::styled(text, style).no_wrap(),
                    ]
                }
            } else {
                let (text, style) = Self::value_display(&row.value);
                let style = if in_red {
                    red_st
                } else if in_yellow {
                    yellow_st
                } else {
                    style
                };
                vec![
                    Span::new(" ").no_wrap(),
                    Span::styled(text, style).no_wrap(),
                ]
            };

            let mut line = vec![cursor_span, indent_span, icon_span];
            line.extend(key_spans);
            line.extend(value_spans);
            lines.push(line);

            // Inline confirm delete row
            if let Mode::ConfirmDelete { row: dr, choice } = &self.mode {
                if *dr == vis {
                    // Render "Delete x? No Yes" inline
                    let prompt = Span::styled(
                        format!("  Delete {}? ", row.key),
                        Style::new().color(Color::Red),
                    )
                    .no_wrap();
                    let opts: Vec<Span> = choice
                        .draw(&RenderContext {
                            focused_id: if focused {
                                Some(self.base.id().to_string())
                            } else {
                                None
                            },
                            ..ctx.clone()
                        })
                        .lines
                        .into_iter()
                        .flatten()
                        .collect();
                    let mut confirm_line = vec![Span::new("  ").no_wrap(), prompt];
                    confirm_line.extend(opts);
                    lines.push(confirm_line);
                }
            }

            // Inline insert row (after active)
            if let Mode::InsertType {
                after_row,
                key_input,
                type_select,
            } = &self.mode
            {
                if *after_row == vis {
                    let key_val = key_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    let type_val = type_select
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    let insert_line = vec![
                        Span::new("  ").no_wrap(),
                        Span::styled(
                            format!("  key: [ {key_val}_ ]"),
                            Style::new().color(Color::Cyan),
                        )
                        .no_wrap(),
                        Span::new("  type: ").no_wrap(),
                        Span::styled(format!("< {type_val} >"), Style::new().color(Color::Yellow))
                            .no_wrap(),
                        Span::styled("  Tab to switch  Enter confirm", inactive).no_wrap(),
                    ];
                    lines.push(insert_line);
                }
            }

            if let Mode::InsertValue {
                after_row,
                key,
                value_input,
                ..
            } = &self.mode
            {
                if *after_row == vis {
                    let val = value_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    let insert_line = vec![
                        Span::new("  ").no_wrap(),
                        Span::styled(
                            format!("  {key}: [ {val}_ ]"),
                            Style::new().color(Color::Cyan),
                        )
                        .no_wrap(),
                        Span::styled("  Enter confirm  Esc cancel", inactive).no_wrap(),
                    ];
                    lines.push(insert_line);
                }
            }
        }

        // Scroll footer
        if let Some(text) = self.scroll.footer(total) {
            lines.push(vec![Span::styled(text, inactive).no_wrap()]);
        }

        // Hint bar
        if focused {
            let hint = match &self.mode {
                Mode::Normal => "  ↑↓ nav  Space expand  Enter edit  i insert  d delete  m move",
                Mode::EditValue { .. } | Mode::EditKey { .. } => {
                    "  Enter confirm  Tab key↔val  Esc cancel"
                }
                Mode::InsertType { .. } => "  Tab key↔type  ←→ type  Enter confirm  Esc cancel",
                Mode::InsertValue { .. } => "  Enter confirm  Esc cancel",
                Mode::ConfirmDelete { .. } => "  ←→ No/Yes  Enter confirm",
                Mode::Move { .. } => "  ↑↓ move  m or Esc done",
            };
            lines.push(vec![Span::styled(hint, inactive).no_wrap()]);
        }

        DrawOutput { lines }
    }
}

// ── Interactive ───────────────────────────────────────────────────────────────

impl Interactive for ObjectEditor {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match &self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::EditValue { .. } => self.handle_edit_value(key),
            Mode::EditKey { .. } => self.handle_edit_key(key),
            Mode::InsertType { .. } => self.handle_insert_type(key),
            Mode::InsertValue { .. } => self.handle_insert_value(key),
            Mode::ConfirmDelete { .. } => self.handle_confirm_delete(key),
            Mode::Move { .. } => self.handle_move(key),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(self.value.clone())
    }

    fn set_value(&mut self, value: Value) {
        self.value = value;
        self.expanded.clear();
        self.expand_all_top_level();
        self.rebuild();
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        None
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueChanged { change } if change.target.as_str() == self.base.id() => {
                self.set_value(change.value.clone());
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }
}

// ── Key handlers ──────────────────────────────────────────────────────────────

impl ObjectEditor {
    fn handle_normal(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }
        match key.code {
            KeyCode::Up => {
                self.move_cursor(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.move_cursor(1);
                InteractionResult::handled()
            }
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Left => {
                self.toggle_expand();
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                self.start_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Char('r') => {
                self.start_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Char('i') => {
                self.start_insert();
                InteractionResult::handled()
            }
            KeyCode::Char('d') => {
                self.start_delete();
                InteractionResult::handled()
            }
            KeyCode::Char('m') => {
                self.start_move();
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn handle_edit_value(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                self.commit_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Tab => {
                self.commit_edit_value();
                self.start_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            _ => {
                let _r = self.edit_input.on_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                self.commit_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Tab => {
                self.commit_edit_key();
                self.start_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            _ => {
                self.edit_input.on_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_type(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                self.commit_insert_type();
                InteractionResult::handled()
            }
            KeyCode::Tab => {
                // Toggle focus between key_input and type_select
                // Simple: Tab cycles key→type→key
                // We track focus by a flag embedded in InsertType
                // For simplicity: first Tab goes to type, second Enter confirms
                InteractionResult::handled()
            }
            KeyCode::Left | KeyCode::Right => {
                if let Mode::InsertType { type_select, .. } = &mut self.mode {
                    type_select.on_key(key);
                }
                InteractionResult::handled()
            }
            _ => {
                if let Mode::InsertType { key_input, .. } = &mut self.mode {
                    key_input.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_value(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                self.commit_insert_value();
                InteractionResult::handled()
            }
            _ => {
                if let Mode::InsertValue { value_input, .. } = &mut self.mode {
                    value_input.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                let confirmed = if let Mode::ConfirmDelete { choice, .. } = &self.mode {
                    choice
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .map(|s| s == "Yes")
                        .unwrap_or(false)
                } else {
                    false
                };
                self.commit_delete(confirmed);
                InteractionResult::handled()
            }
            _ => {
                if let Mode::ConfirmDelete { choice, .. } = &mut self.mode {
                    choice.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_move(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc | KeyCode::Char('m') => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Up => {
                self.move_node(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.move_node(1);
                InteractionResult::handled()
            }
            _ => InteractionResult::handled(),
        }
    }
}
