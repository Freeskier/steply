use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;

use crate::core::NodeId;
use crate::core::search::fuzzy::match_text;
use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValuePath, ValueTarget};

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use inline_key_value::{InlineKeyValueEditor, InlineKeyValueFocus};
use unicode_width::UnicodeWidthChar;

#[derive(Clone)]
pub struct CustomInsertType {
    name: String,
    parser: Arc<dyn Fn(&str) -> Value + Send + Sync>,
}

impl CustomInsertType {
    pub fn new(
        name: impl Into<String>,
        parser: impl Fn(&str) -> Value + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            parser: Arc::new(parser),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn parse(&self, text: &str) -> Value {
        (self.parser)(text)
    }
}

#[derive(Clone)]
struct ObjNode {
    key: String,
    value: Value,

    path: String,

    is_index: bool,
    is_placeholder: bool,
    placeholder_parent: Option<String>,
}

impl TreeItemLabel for ObjNode {
    fn label(&self) -> &str {
        &self.key
    }

    fn search_text(&self) -> Cow<'_, str> {
        let value = match &self.value {
            Value::Text(s) => s.clone(),
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::None => "null".to_string(),
            Value::Object(map) => format!("{{{}}}", map.len()),
            Value::List(list) => format!("[{}]", list.len()),
        };
        Cow::Owned(format!("{} {}", self.key, value))
    }
}

enum Mode {
    Normal,
    EditValue {
        vis: usize,
        key_value: InlineKeyValueEditor,
    },
    EditKey {
        vis: usize,
        key_value: InlineKeyValueEditor,
    },
    InsertType {
        after_vis: usize,
        key_value: InlineKeyValueEditor,
    },
    InsertValue {
        after_vis: usize,
        value_type: InsertValueType,
        key_value: InlineKeyValueEditor,
    },
    ConfirmDelete {
        vis: usize,
        select: SelectInput,
    },
    Move {
        vis: usize,
    },
}

#[derive(Clone, Copy)]
enum InsertValueType {
    Text,
    Number,
    Custom(usize),
}

#[derive(Debug, Clone)]
enum InsertPlacement {
    Start,
    End,
    Before(String),
    After(String),
}

#[derive(Debug, Clone)]
struct MovePlan {
    target_vis: usize,
    source_path: String,
    dest_parent: String,
    placement: InsertPlacement,
}

pub struct ObjectEditor {
    base: WidgetBase,
    value: Value,
    expanded: HashSet<String>,
    array_item_names: HashMap<String, String>,
    tree: TreeView<ObjNode>,
    filter: TextInput,
    filter_visible: bool,
    filter_focus: bool,
    custom_insert_types: Vec<CustomInsertType>,
    mode: Mode,
    submit_target: Option<ValueTarget>,
}

impl ObjectEditor {
    fn spans_width(spans: &[Span]) -> u16 {
        spans
            .iter()
            .flat_map(|span| span.text.chars())
            .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0) as u16)
            .sum()
    }

    fn tree_icon_slot(tree_line: &[Span]) -> usize {
        tree_line
            .iter()
            .rposition(|span| matches!(span.text.as_str(), "▶ " | "▼ " | "⟳ " | "  "))
            .unwrap_or(0)
    }

    fn tree_content_start(tree_line: &[Span]) -> usize {
        Self::tree_icon_slot(tree_line)
            .saturating_add(1)
            .min(tree_line.len())
    }

    fn tree_insert_prefix(tree_line: &[Span]) -> Vec<Span> {
        let icon_pos = Self::tree_icon_slot(tree_line);
        let mut prefix: Vec<Span> = tree_line[..icon_pos].to_vec();
        let icon_style = tree_line
            .get(icon_pos)
            .map(|span| span.style)
            .unwrap_or_else(|| Style::new().color(Color::DarkGrey));
        prefix.push(Span::styled("  ", icon_style).no_wrap());
        prefix
    }

    fn tree_prefix_width(tree_line: &[Span]) -> u16 {
        let start = Self::tree_content_start(tree_line);
        Self::spans_width(&tree_line[..start])
    }

    fn headers_row_offset(&self) -> u16 {
        let mut row: u16 = 0;
        if !self.base.label().is_empty() {
            row = row.saturating_add(1);
        }
        if self.filter_visible {
            row = row.saturating_add(1);
        }
        row
    }

    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let tree_id = format!("{id}__tree");
        let filter_id = format!("{id}__filter");
        let mut this = Self {
            base: WidgetBase::new(id, label),
            value: Value::Object(IndexMap::new()),
            expanded: HashSet::new(),
            array_item_names: HashMap::new(),
            tree: TreeView::new(tree_id, "", Vec::new()).with_show_label(false),
            filter: TextInput::new(filter_id, ""),
            filter_visible: false,
            filter_focus: false,
            custom_insert_types: Vec::new(),
            mode: Mode::Normal,
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
        let id = format!("{}_tree", self.base.id());
        let nodes = std::mem::take(self.tree.nodes_mut());
        self.tree = TreeView::new(id, "", nodes)
            .with_show_label(false)
            .with_max_visible(n);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn with_custom_insert_type(mut self, custom: CustomInsertType) -> Self {
        self.custom_insert_types.push(custom);
        self
    }

    pub fn with_custom_insert_types(mut self, custom: Vec<CustomInsertType>) -> Self {
        self.custom_insert_types.extend(custom);
        self
    }

    fn insert_type_options(&self) -> Vec<String> {
        let mut options = vec![
            "text".to_string(),
            "number".to_string(),
            "object".to_string(),
            "array".to_string(),
        ];
        for custom in &self.custom_insert_types {
            options.push(custom.name().to_string());
        }
        options
    }

    fn resolve_insert_value_type(&self, value_type: &str) -> InsertValueType {
        match value_type {
            "number" => InsertValueType::Number,
            "text" => InsertValueType::Text,
            _ => self
                .custom_insert_types
                .iter()
                .position(|custom| custom.name() == value_type)
                .map(InsertValueType::Custom)
                .unwrap_or(InsertValueType::Text),
        }
    }

    fn expand_all_top_level(&mut self) {
        match &self.value {
            Value::Object(map) => {
                for k in map.keys() {
                    self.expanded
                        .insert(ValuePath::new(vec![PathSegment::Key(k.clone())]).to_string());
                }
            }
            Value::List(arr) => {
                for i in 0..arr.len() {
                    self.expanded
                        .insert(ValuePath::new(vec![PathSegment::Index(i)]).to_string());
                }
            }
            _ => {}
        }
    }

    fn rebuild(&mut self) {
        let nodes = Self::build_nodes(&self.value, &self.expanded, 0, &ValuePath::empty());
        self.tree.set_nodes(nodes);
        self.tree.set_filter_query(self.filter_query());
    }

    fn filter_query(&self) -> String {
        self.filter
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    fn toggle_filter_visibility(&mut self) {
        self.filter_visible = !self.filter_visible;
        if self.filter_visible {
            self.filter_focus = true;
            return;
        }
        self.filter_focus = false;
        self.filter.set_value(Value::Text(String::new()));
        self.tree.clear_filter();
    }

    fn apply_filter_from_input(&mut self) {
        self.tree.set_filter_query(self.filter_query());
    }

    fn pending_insert_value_error(&self) -> Option<String> {
        let Mode::InsertValue { key_value, .. } = &self.mode else {
            return None;
        };
        let key = key_value.key();
        let value = key_value.value_text();
        if !key.trim().is_empty() && value.trim().is_empty() {
            return Some("value cannot be empty".to_string());
        }
        None
    }

    fn child_context(&self, ctx: &RenderContext, focused_id: Option<String>) -> RenderContext {
        RenderContext {
            focused_id,
            terminal_size: ctx.terminal_size,
            visible_errors: ctx.visible_errors.clone(),
            invalid_hidden: ctx.invalid_hidden.clone(),
            completion_menus: ctx.completion_menus.clone(),
        }
    }
}

mod actions;
mod inline_key_value;
mod interaction;
mod model;
mod render;
