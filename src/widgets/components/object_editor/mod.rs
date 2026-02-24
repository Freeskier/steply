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
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::filter;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use inline_key_value::{CustomValueInput, InlineKeyValueEditor, InlineKeyValueFocus};
use unicode_width::UnicodeWidthChar;

#[derive(Clone)]
pub struct InsertType {
    name: String,
    value_input: Arc<dyn Fn(String) -> CustomValueInput + Send + Sync>,
    parser: Arc<dyn Fn(&str) -> Value + Send + Sync>,
}

impl InsertType {
    pub fn custom<I>(
        name: impl Into<String>,
        value_input: impl Fn(String) -> I + Send + Sync + 'static,
        parser: impl Fn(&str) -> Value + Send + Sync + 'static,
    ) -> Self
    where
        I: Into<CustomValueInput> + 'static,
    {
        Self {
            name: name.into(),
            value_input: Arc::new(move |id| value_input(id).into()),
            parser: Arc::new(parser),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn parse(&self, text: &str) -> Value {
        (self.parser)(text)
    }

    fn new_editor(&self, id: String, key: String) -> InlineKeyValueEditor {
        let value_id = format!("{id}__value");
        let mut editor = InlineKeyValueEditor::new_custom(id, "", (self.value_input)(value_id))
            .with_default_key(key)
            .with_default_value("");
        editor.set_focus(InlineKeyValueFocus::Value);
        editor
    }
}

#[derive(Clone)]
struct ObjectTreeNode {
    key: String,
    value: Value,

    path: String,

    is_index: bool,
    is_placeholder: bool,
    placeholder_parent: Option<String>,
}

impl TreeItemLabel for ObjectTreeNode {
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
        visible_index: usize,
        key_value: InlineKeyValueEditor,
    },
    EditKey {
        visible_index: usize,
        key_value: InlineKeyValueEditor,
    },
    InsertType {
        after_visible_index: usize,
        key_value: InlineKeyValueEditor,
    },
    InsertValue {
        after_visible_index: usize,
        value_type: InsertValueType,
        key_value: InlineKeyValueEditor,
    },
    ConfirmDelete {
        visible_index: usize,
        select: SelectInput,
    },
    Move {
        visible_index: usize,
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
    target_visible_index: usize,
    source_path: String,
    dest_parent: String,
    placement: InsertPlacement,
}

#[derive(Debug)]
struct InsertSpec {
    parent_path: String,
    key: String,
    was_index: bool,
    source_name: Option<String>,
    value: Value,
    placement: InsertPlacement,
    source_parent: String,
}

pub struct ObjectEditor {
    base: WidgetBase,
    value: Value,
    expanded: HashSet<String>,
    array_item_names: HashMap<String, String>,
    tree: TreeView<ObjectTreeNode>,
    filter: filter::FilterController,
    insert_types: Vec<InsertType>,
    mode: Mode,
    submit_target: Option<ValueTarget>,
}

impl ObjectEditor {
    fn tree_id(base_id: &str) -> String {
        format!("{base_id}__tree")
    }

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
        if self.filter.is_visible() {
            row = row.saturating_add(1);
        }
        row
    }

    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let tree_id = Self::tree_id(id.as_str());
        let filter_id = format!("{id}__filter");
        let mut this = Self {
            base: WidgetBase::new(id, label),
            value: Value::Object(IndexMap::new()),
            expanded: HashSet::new(),
            array_item_names: HashMap::new(),
            tree: TreeView::new(tree_id, "", Vec::new()).with_show_label(false),
            filter: filter::FilterController::new(filter_id),
            insert_types: Vec::new(),
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
        let id = Self::tree_id(self.base.id());
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

    pub fn with_insert_type(mut self, insert_type: InsertType) -> Self {
        self.insert_types.push(insert_type);
        self
    }

    pub fn with_insert_types(mut self, insert_types: Vec<InsertType>) -> Self {
        self.insert_types.extend(insert_types);
        self
    }

    fn insert_type_options(&self) -> Vec<String> {
        let mut options = vec![
            "text".to_string(),
            "number".to_string(),
            "object".to_string(),
            "array".to_string(),
        ];
        for insert_type in &self.insert_types {
            options.push(insert_type.name().to_string());
        }
        options
    }

    fn resolve_insert_value_type(&self, value_type: &str) -> InsertValueType {
        match value_type {
            "number" => InsertValueType::Number,
            "text" => InsertValueType::Text,
            _ => self
                .insert_types
                .iter()
                .position(|insert_type| insert_type.name() == value_type)
                .map(InsertValueType::Custom)
                .unwrap_or(InsertValueType::Text),
        }
    }

    fn insert_value_editor(
        &self,
        editor_id: String,
        key: String,
        value_type: InsertValueType,
    ) -> InlineKeyValueEditor {
        match value_type {
            InsertValueType::Custom(index) => self
                .insert_types
                .get(index)
                .map(|insert_type| insert_type.new_editor(editor_id.clone(), key.clone()))
                .unwrap_or_else(|| {
                    let mut editor = InlineKeyValueEditor::new_text(editor_id, "")
                        .with_default_key(key)
                        .with_default_value("");
                    editor.set_focus(InlineKeyValueFocus::Value);
                    editor
                }),
            InsertValueType::Text | InsertValueType::Number => {
                let mut editor = InlineKeyValueEditor::new_text(editor_id, "")
                    .with_default_key(key)
                    .with_default_value("");
                editor.set_focus(InlineKeyValueFocus::Value);
                editor
            }
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
        self.filter.query()
    }

    fn toggle_filter_visibility(&mut self) {
        let visible = self.filter.toggle_visibility(false);
        if visible {
            return;
        }
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
}

mod actions;
mod inline_key_value;
mod interaction;
mod model;
mod render;
