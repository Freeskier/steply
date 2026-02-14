use std::collections::HashSet;

use indexmap::IndexMap;

use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};

// ── ObjNode — one row ────────────────────────────────────────────────────────

#[derive(Clone)]
struct ObjNode {
    key: String,
    value: Value,
    /// Full dot-path from root, e.g. "address.city".
    path: String,
    /// True for List children (key is the index string).
    is_index: bool,
}

impl TreeItemLabel for ObjNode {
    /// TreeView uses this only for its own default rendering;
    /// ObjectEditor replaces the label span in draw().
    fn label(&self) -> &str {
        &self.key
    }
}

// ── Modes ────────────────────────────────────────────────────────────────────

enum Mode {
    Normal,
    EditValue {
        vis: usize,
    },
    EditKey {
        vis: usize,
    },
    InsertType {
        after_vis: usize,
        key_input: TextInput,
        type_select: SelectInput,
        focus_key: bool,
    },
    InsertValue {
        after_vis: usize,
        key: String,
        value_type: InsertValueType,
        value_input: TextInput,
    },
    ConfirmDelete {
        vis: usize,
        choice: ChoiceInput,
    },
    Move {
        vis: usize,
    },
}

#[derive(Clone, Copy)]
enum InsertValueType {
    Text,
    Number,
}

// ── ObjectEditor ─────────────────────────────────────────────────────────────

pub struct ObjectEditor {
    base: WidgetBase,
    value: Value,
    expanded: HashSet<String>,
    tree: TreeView<ObjNode>,
    mode: Mode,
    /// Reused TextInput for inline key/value editing.
    edit_input: TextInput,
    submit_target: Option<String>,
}

impl ObjectEditor {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let edit_id = format!("{id}__edit");
        let tree_id = format!("{id}__tree");
        let mut this = Self {
            base: WidgetBase::new(id, label),
            value: Value::Object(IndexMap::new()),
            expanded: HashSet::new(),
            tree: TreeView::new(tree_id, "", Vec::new()).with_show_label(false),
            mode: Mode::Normal,
            edit_input: TextInput::new(edit_id, ""),
            submit_target: None,
        };
        this.rebuild();
        this
    }

    pub fn with_value(mut self, value: Value) -> Self {
        self.value = value;
        self.expand_all_top_level();
        self.rebuild();
        self
    }

    pub fn with_max_visible(mut self, n: usize) -> Self {
        // Re-create tree with max_visible (TreeView builder pattern requires this).
        let id = format!("{}_tree", self.base.id());
        let nodes = std::mem::take(self.tree.nodes_mut());
        self.tree = TreeView::new(id, "", nodes)
            .with_show_label(false)
            .with_max_visible(n);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    // ── Expand helpers ────────────────────────────────────────────────────────

    fn expand_all_top_level(&mut self) {
        match &self.value {
            Value::Object(map) => {
                for k in map.keys() {
                    self.expanded.insert(k.clone());
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

    // ── Rebuild: Value → TreeNode list ────────────────────────────────────────

    fn rebuild(&mut self) {
        let nodes = Self::build_nodes(&self.value, &self.expanded, 0, "");
        self.tree.set_nodes(nodes);
    }

    fn build_nodes(
        value: &Value,
        expanded: &HashSet<String>,
        depth: usize,
        prefix: &str,
    ) -> Vec<TreeNode<ObjNode>> {
        let mut out = Vec::new();
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    let is_container = matches!(child, Value::Object(_) | Value::List(_));
                    let is_exp = is_container && expanded.contains(&path);
                    let mut node = TreeNode::new(
                        ObjNode {
                            key: key.clone(),
                            value: child.clone(),
                            path: path.clone(),
                            is_index: false,
                        },
                        depth,
                        is_container,
                    );
                    if is_exp {
                        node.expanded = true;
                        node.children_loaded = true;
                    }
                    out.push(node);
                    if is_exp {
                        out.extend(Self::build_nodes(child, expanded, depth + 1, &path));
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
                    let is_exp = is_container && expanded.contains(&path);
                    let mut node = TreeNode::new(
                        ObjNode {
                            key: key.clone(),
                            value: child.clone(),
                            path: path.clone(),
                            is_index: true,
                        },
                        depth,
                        is_container,
                    );
                    if is_exp {
                        node.expanded = true;
                        node.children_loaded = true;
                    }
                    out.push(node);
                    if is_exp {
                        out.extend(Self::build_nodes(child, expanded, depth + 1, &path));
                    }
                }
            }
            _ => {}
        }
        out
    }

    // ── Accessors using tree.visible() ────────────────────────────────────────

    fn active_vis(&self) -> usize {
        self.tree.active_visible_index()
    }

    fn active_obj(&self) -> Option<&ObjNode> {
        self.tree.active_node().map(|n| &n.item)
    }

    fn path_at(&self, vis: usize) -> String {
        let visible = self.tree.visible();
        visible
            .get(vis)
            .and_then(|&idx| self.tree.nodes().get(idx))
            .map(|n| n.item.path.clone())
            .unwrap_or_default()
    }

    fn vis_of_path(&self, path: &str) -> Option<usize> {
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        visible
            .iter()
            .position(|&idx| nodes.get(idx).map(|n| n.item.path == path).unwrap_or(false))
    }

    fn subtree_vis_range(&self, vis: usize) -> std::ops::Range<usize> {
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        if vis >= visible.len() {
            return vis..vis;
        }
        let depth = nodes[visible[vis]].depth;
        let end = visible[vis + 1..]
            .iter()
            .position(|&idx| nodes.get(idx).map(|n| n.depth <= depth).unwrap_or(true))
            .map(|p| vis + 1 + p)
            .unwrap_or(visible.len());
        vis + 1..end
    }

    // ── Value mutation helpers ────────────────────────────────────────────────

    fn value_at_path_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        if path.is_empty() {
            return Some(root);
        }
        let mut cur = root;
        for part in path.split('.') {
            match cur {
                Value::Object(map) => cur = map.get_mut(part)?,
                Value::List(arr) => {
                    let i: usize = part.parse().ok()?;
                    cur = arr.get_mut(i)?;
                }
                _ => return None,
            }
        }
        Some(cur)
    }

    fn parent_path(path: &str) -> &str {
        match path.rfind('.') {
            Some(p) => &path[..p],
            None => "",
        }
    }

    fn leaf_key(path: &str) -> &str {
        match path.rfind('.') {
            Some(p) => &path[p + 1..],
            None => path,
        }
    }

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

    // ── Toggle expand ─────────────────────────────────────────────────────────

    fn toggle_expand(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if !matches!(obj.value, Value::Object(_) | Value::List(_)) {
            return;
        }
        let path = obj.path.clone();
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        self.rebuild();
    }

    // ── Edit value ────────────────────────────────────────────────────────────

    fn start_edit_value(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if matches!(obj.value, Value::Object(_) | Value::List(_)) {
            return;
        }
        let text = obj.value.to_text_scalar().unwrap_or_else(|| "null".into());
        let vis = self.active_vis();
        self.edit_input.set_value(Value::Text(text));
        self.mode = Mode::EditValue { vis };
    }

    fn commit_edit_value(&mut self) {
        let Mode::EditValue { vis } = self.mode else {
            return;
        };
        let text = self
            .edit_input
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default();
        let new_val = Self::parse_scalar(&text);
        let path = self.path_at(vis);
        let ppath = Self::parent_path(&path).to_string();
        let key = Self::leaf_key(&path).to_string();
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            match parent {
                Value::Object(map) => {
                    map.insert(key, new_val);
                }
                Value::List(arr) => {
                    if let Ok(i) = key.parse::<usize>() {
                        if i < arr.len() {
                            arr[i] = new_val;
                        }
                    }
                }
                _ => {}
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    fn start_edit_key(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if obj.is_index {
            return;
        }
        let key = obj.key.clone();
        let vis = self.active_vis();
        self.edit_input.set_value(Value::Text(key));
        self.mode = Mode::EditKey { vis };
    }

    fn commit_edit_key(&mut self) {
        let Mode::EditKey { vis } = self.mode else {
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
        let path = self.path_at(vis);
        let ppath = Self::parent_path(&path).to_string();
        let old_key = Self::leaf_key(&path).to_string();
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            if let Value::Object(map) = parent {
                if let Some(val) = map.shift_remove(&old_key) {
                    map.insert(new_key, val);
                }
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    // ── Insert ────────────────────────────────────────────────────────────────

    fn start_insert(&mut self) {
        let after_vis = self.active_vis();
        self.mode = Mode::InsertType {
            after_vis,
            key_input: TextInput::new(format!("{}_ik", self.base.id()), ""),
            type_select: SelectInput::new(
                format!("{}_it", self.base.id()),
                "",
                vec![
                    "text".into(),
                    "number".into(),
                    "object".into(),
                    "array".into(),
                ],
            ),
            focus_key: true,
        };
    }

    fn commit_insert_type(&mut self) {
        let Mode::InsertType {
            after_vis,
            ref key_input,
            ref type_select,
            ..
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
        let av = after_vis;
        let k = key.clone();
        let tv = type_val.clone();

        match tv.as_str() {
            "object" | "array" => {
                let new_val = if tv == "object" {
                    Value::Object(IndexMap::new())
                } else {
                    Value::List(Vec::new())
                };
                self.do_insert(av, k, new_val);
                self.mode = Mode::Normal;
                self.rebuild();
            }
            vt => {
                let value_type = if vt == "number" {
                    InsertValueType::Number
                } else {
                    InsertValueType::Text
                };
                self.mode = Mode::InsertValue {
                    after_vis: av,
                    key: k,
                    value_type,
                    value_input: TextInput::new(format!("{}_iv", self.base.id()), ""),
                };
            }
        }
    }

    fn commit_insert_value(&mut self) {
        let Mode::InsertValue {
            after_vis,
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
            InsertValueType::Text => Self::parse_scalar(&text),
        };
        let av = after_vis;
        let k = key.clone();
        self.do_insert(av, k, new_val);
        self.mode = Mode::Normal;
        self.rebuild();
    }

    fn do_insert(&mut self, after_vis: usize, new_key: String, new_val: Value) {
        let path = self.path_at(after_vis);
        let ppath = Self::parent_path(&path).to_string();
        let sib_key = Self::leaf_key(&path).to_string();
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            match parent {
                Value::Object(map) => {
                    map.insert(new_key, new_val);
                }
                Value::List(arr) => {
                    let idx = sib_key.parse::<usize>().unwrap_or(arr.len());
                    arr.insert(idx + 1, new_val);
                }
                _ => {}
            }
        } else {
            match &mut self.value {
                Value::Object(map) => {
                    map.insert(new_key, new_val);
                }
                Value::List(arr) => {
                    arr.push(new_val);
                }
                _ => {}
            }
        }
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    fn start_delete(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        let vis = self.active_vis();
        let label = obj.key.clone();
        let choice = ChoiceInput::new(
            format!("{}_cd", self.base.id()),
            format!("Delete {label}?"),
            vec!["No".into(), "Yes".into()],
        )
        .with_bullets(false);
        self.mode = Mode::ConfirmDelete { vis, choice };
    }

    fn commit_delete(&mut self, confirmed: bool) {
        let Mode::ConfirmDelete { vis, .. } = self.mode else {
            return;
        };
        if confirmed {
            let path = self.path_at(vis);
            let ppath = Self::parent_path(&path).to_string();
            let key = Self::leaf_key(&path).to_string();
            if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
                match parent {
                    Value::Object(map) => {
                        map.shift_remove(&key);
                    }
                    Value::List(arr) => {
                        if let Ok(i) = key.parse::<usize>() {
                            if i < arr.len() {
                                arr.remove(i);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    // ── Move ─────────────────────────────────────────────────────────────────

    fn start_move(&mut self) {
        let vis = self.active_vis();
        self.mode = Mode::Move { vis };
    }

    fn move_node(&mut self, delta: isize) {
        let current_vis = match self.mode {
            Mode::Move { vis } => vis,
            _ => return,
        };
        let total = self.tree.visible().len();
        if total <= 1 {
            return;
        }
        let target_vis =
            ((current_vis as isize + delta + total as isize) % total as isize) as usize;

        let cur_path = self.path_at(current_vis);
        let tgt_path = self.path_at(target_vis);
        let cur_parent = Self::parent_path(&cur_path).to_string();
        let tgt_parent = Self::parent_path(&tgt_path).to_string();
        let cur_key = Self::leaf_key(&cur_path).to_string();
        let tgt_key = Self::leaf_key(&tgt_path).to_string();

        if cur_parent == tgt_parent {
            if let Some(parent) = Self::value_at_path_mut(&mut self.value, &cur_parent) {
                match parent {
                    Value::List(arr) => {
                        let ci = cur_key.parse::<usize>().unwrap_or(0);
                        let ti = tgt_key.parse::<usize>().unwrap_or(0);
                        if ci < arr.len() && ti < arr.len() {
                            arr.swap(ci, ti);
                        }
                    }
                    Value::Object(map) => {
                        if let (Some(ci), Some(ti)) = (
                            map.get_index_of(cur_key.as_str()),
                            map.get_index_of(tgt_key.as_str()),
                        ) {
                            map.swap_indices(ci, ti);
                        }
                    }
                    _ => {}
                }
            }
        }

        self.rebuild();
        let new_vis = self.vis_of_path(&cur_path).unwrap_or(target_vis);
        // Sync tree's active cursor
        while self.tree.active_visible_index() < new_vis {
            if !self.tree.move_active(1) {
                break;
            }
        }
        while self.tree.active_visible_index() > new_vis {
            if !self.tree.move_active(-1) {
                break;
            }
        }
        self.mode = Mode::Move { vis: new_vis };
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

    fn row_spans(&self, vis: usize, obj: &ObjNode, red: bool, yellow: bool) -> Vec<Span> {
        let red_st = Style::new().color(Color::Red);
        let yellow_st = Style::new().color(Color::Yellow);
        let key_st = Style::new().color(Color::White).bold();
        let key_dim = Style::new().color(Color::DarkGrey);
        let cyan_st = Style::new().color(Color::Cyan);

        let key_style = if red {
            red_st
        } else if yellow {
            yellow_st
        } else if obj.is_index {
            key_dim
        } else {
            key_st
        };

        let key_part: Vec<Span> = if let Mode::EditKey { vis: ev } = &self.mode {
            if *ev == vis {
                let v = self
                    .edit_input
                    .value()
                    .and_then(|v| v.to_text_scalar())
                    .unwrap_or_default();
                vec![Span::styled(format!("[ {v}_ ]"), cyan_st).no_wrap()]
            } else {
                vec![Span::styled(format!("{}:", obj.key), key_style).no_wrap()]
            }
        } else {
            vec![Span::styled(format!("{}:", obj.key), key_style).no_wrap()]
        };

        let val_part: Vec<Span> = if let Mode::EditValue { vis: ev } = &self.mode {
            if *ev == vis {
                let v = self
                    .edit_input
                    .value()
                    .and_then(|v| v.to_text_scalar())
                    .unwrap_or_default();
                vec![
                    Span::new(" ").no_wrap(),
                    Span::styled(format!("[ {v}_ ]"), cyan_st).no_wrap(),
                ]
            } else {
                let (text, style) = Self::value_display(&obj.value);
                let style = if red {
                    red_st
                } else if yellow {
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
            let (text, style) = Self::value_display(&obj.value);
            let style = if red {
                red_st
            } else if yellow {
                yellow_st
            } else {
                style
            };
            vec![
                Span::new(" ").no_wrap(),
                Span::styled(text, style).no_wrap(),
            ]
        };

        let mut spans = key_part;
        spans.extend(val_part);
        spans
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

        // Highlight ranges
        let red_range: Option<std::ops::Range<usize>> = match &self.mode {
            Mode::ConfirmDelete { vis, .. } => Some(self.subtree_vis_range(*vis)),
            _ => None,
        };
        let yellow_vis: Option<usize> = match &self.mode {
            Mode::Move { vis } => Some(*vis),
            _ => None,
        };

        // Get tree lines (cursor ❯, indent, icon ▼/▶, label) from TreeView.
        // We'll swap out the label span with our key:value spans.
        let tree_lines = self.tree.render_lines(focused);
        let (start, _end) = self.tree.visible_range();
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();

        let mut lines: Vec<Vec<Span>> = Vec::new();

        // Label
        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        for (line_idx, mut tree_line) in tree_lines.into_iter().enumerate() {
            let vis = start + line_idx;
            // Footer line from TreeView (no corresponding node)
            if vis >= visible.len() {
                lines.push(tree_line);
                continue;
            }

            let node_idx = visible[vis];
            let obj = &nodes[node_idx].item;

            let in_red = red_range
                .as_ref()
                .map(|r| r.contains(&vis))
                .unwrap_or(false);
            let in_yellow = yellow_vis == Some(vis);

            // Replace label span (last) with our key:value spans
            tree_line.pop();

            // Tint structural spans (cursor/indent/icon) for red/yellow rows
            if in_red || in_yellow {
                let tint = if in_red {
                    Style::new().color(Color::Red)
                } else {
                    Style::new().color(Color::Yellow)
                };
                for span in tree_line.iter_mut() {
                    if !span.text.trim().is_empty() {
                        span.style = tint;
                    }
                }
            }

            tree_line.extend(self.row_spans(vis, obj, in_red, in_yellow));
            lines.push(tree_line);

            // Inline confirm-delete row (shown right after the target row)
            if let Mode::ConfirmDelete { vis: dv, choice } = &self.mode {
                if *dv == vis {
                    let prompt = Span::styled(
                        format!("  Delete {}? ", obj.key),
                        Style::new().color(Color::Red),
                    )
                    .no_wrap();
                    let choice_ctx = RenderContext {
                        focused_id: if focused {
                            Some(self.base.id().to_string())
                        } else {
                            None
                        },
                        ..ctx.clone()
                    };
                    let choice_spans: Vec<Span> = choice
                        .draw(&choice_ctx)
                        .lines
                        .into_iter()
                        .flatten()
                        .collect();
                    let mut row = vec![Span::new("  ").no_wrap(), prompt];
                    row.extend(choice_spans);
                    lines.push(row);
                }
            }

            // Inline insert-type row
            if let Mode::InsertType {
                after_vis,
                key_input,
                type_select,
                focus_key,
            } = &self.mode
            {
                if *after_vis == vis {
                    let kv = key_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    let cyan = Style::new().color(Color::Cyan);
                    let dim = Style::new().color(Color::DarkGrey);
                    let key_span = if *focus_key {
                        Span::styled(format!("[ {kv}_ ]"), cyan).no_wrap()
                    } else {
                        Span::styled(format!("[ {kv} ]"), dim).no_wrap()
                    };
                    let tv = type_select
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_else(|| "text".into());
                    let type_span = if !focus_key {
                        Span::styled(format!("‹ {tv} ›"), cyan).no_wrap()
                    } else {
                        Span::styled(format!("‹ {tv} ›"), dim).no_wrap()
                    };
                    lines.push(vec![
                        Span::new("    ").no_wrap(),
                        key_span,
                        Span::new(": ").no_wrap(),
                        type_span,
                    ]);
                }
            }

            // Inline insert-value row
            if let Mode::InsertValue {
                after_vis,
                key,
                value_input,
                ..
            } = &self.mode
            {
                if *after_vis == vis {
                    let vv = value_input
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .unwrap_or_default();
                    lines.push(vec![
                        Span::new("    ").no_wrap(),
                        Span::styled(format!("{key}: [ {vv}_ ]"), Style::new().color(Color::Cyan))
                            .no_wrap(),
                        Span::styled("  Enter confirm  Esc cancel", inactive).no_wrap(),
                    ]);
                }
            }
        }

        // Hint bar
        if focused {
            let hint = match &self.mode {
                Mode::Normal => {
                    "  ↑↓ nav  Space expand  Enter edit val  r rename  i insert  d delete  m move"
                }
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
                self.tree.move_active(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.tree.move_active(1);
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
                self.edit_input.on_key(key);
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
                if let Mode::InsertType { focus_key, .. } = &mut self.mode {
                    *focus_key = !*focus_key;
                }
                InteractionResult::handled()
            }
            KeyCode::Left | KeyCode::Right => {
                if let Mode::InsertType {
                    type_select,
                    focus_key,
                    ..
                } = &mut self.mode
                {
                    if !*focus_key {
                        type_select.on_key(key);
                    }
                }
                InteractionResult::handled()
            }
            _ => {
                if let Mode::InsertType {
                    key_input,
                    focus_key,
                    ..
                } = &mut self.mode
                {
                    if *focus_key {
                        key_input.on_key(key);
                    }
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
