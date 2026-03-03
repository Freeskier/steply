use std::sync::Arc;

use indexmap::IndexMap;

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::ValueChange;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::list_policy;
use crate::widgets::shared::validation::decorate_component_validation;
use crate::widgets::shared::value_seed::{normalize_ascii_key, seed_value_from_record};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, InteractiveNode, RenderContext, TextAction, ValidationMode,
};

mod interaction;
mod render;

pub type RepeaterFieldFactory =
    Arc<dyn Fn(String, String) -> Box<dyn InteractiveNode> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterLayout {
    SingleField,
    Stacked,
}

struct RepeaterFieldDef {
    key: String,
    label: String,
    make_input: RepeaterFieldFactory,
}

struct RepeaterRow {
    item: Value,
    fields: Vec<Box<dyn InteractiveNode>>,
}

pub struct Repeater {
    base: WidgetBase,
    fields: Vec<RepeaterFieldDef>,
    rows: Vec<RepeaterRow>,
    active_item: usize,
    active_field: usize,
    layout: RepeaterLayout,
    show_label: bool,
    show_progress: bool,
    header_template: String,
    item_label_path: Option<ValuePath>,
    finished: bool,
    submit_target: Option<ValueTarget>,
}

impl Repeater {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            fields: Vec::new(),
            rows: Vec::new(),
            active_item: 0,
            active_field: 0,
            layout: RepeaterLayout::SingleField,
            show_label: true,
            show_progress: true,
            header_template: "configuring [{index} of {total}] for {item}:".to_string(),
            item_label_path: None,
            finished: false,
            submit_target: None,
        }
    }

    pub fn with_layout(mut self, layout: RepeaterLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn with_show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    pub fn with_header_template(mut self, template: impl Into<String>) -> Self {
        self.header_template = template.into();
        self
    }

    pub fn with_item_label_path(mut self, path: ValuePath) -> Self {
        self.item_label_path = Some(path);
        self
    }

    pub fn with_items(mut self, items: Vec<Value>) -> Self {
        self.set_items(items);
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

    pub fn field<I, F>(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        make_input: F,
    ) -> Self
    where
        I: InteractiveNode + 'static,
        F: Fn(String, String) -> I + Send + Sync + 'static,
    {
        self.push_field(
            key.into(),
            label.into(),
            Arc::new(move |id, label| Box::new(make_input(id, label))),
        );
        self
    }

    pub fn field_auto<I, F>(mut self, label: impl Into<String>, make_input: F) -> Self
    where
        I: InteractiveNode + 'static,
        F: Fn(String, String) -> I + Send + Sync + 'static,
    {
        let label = label.into();
        let key = normalize_ascii_key(label.as_str(), "field");
        self.push_field(
            key,
            label,
            Arc::new(move |id, label| Box::new(make_input(id, label))),
        );
        self
    }

    pub fn field_boxed(
        mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        make_input: RepeaterFieldFactory,
    ) -> Self {
        self.push_field(key.into(), label.into(), make_input);
        self
    }

    fn push_field(&mut self, key: String, label: String, make_input: RepeaterFieldFactory) {
        let unique_key = unique_field_key(self.fields.as_slice(), key.as_str());
        let field_idx = self.fields.len();
        let base_id = self.base.id().to_string();
        self.fields.push(RepeaterFieldDef {
            key: unique_key,
            label: label.clone(),
            make_input: make_input.clone(),
        });

        for (row_idx, row) in self.rows.iter_mut().enumerate() {
            let id = format!("{base_id}__i{row_idx}__f{field_idx}");
            row.fields.push(make_input(id, label.clone()));
        }
        self.clamp_cursor();
    }

    fn set_items(&mut self, items: Vec<Value>) {
        let mut rows = Vec::<RepeaterRow>::with_capacity(items.len());
        for (row_idx, item) in items.into_iter().enumerate() {
            let fields = self
                .fields
                .iter()
                .enumerate()
                .map(|(field_idx, field)| {
                    let id = self.field_id(row_idx, field_idx);
                    (field.make_input)(id, field.label.clone())
                })
                .collect::<Vec<_>>();
            rows.push(RepeaterRow { item, fields });
        }
        self.rows = rows;
        self.finished = false;
        self.clamp_cursor();
    }

    fn field_id(&self, row_idx: usize, field_idx: usize) -> String {
        format!("{}__i{}__f{}", self.base.id(), row_idx, field_idx)
    }

    fn active_row(&self) -> Option<&RepeaterRow> {
        self.rows.get(self.active_item)
    }

    fn active_row_mut(&mut self) -> Option<&mut RepeaterRow> {
        self.rows.get_mut(self.active_item)
    }

    fn active_field_widget(&self) -> Option<&dyn InteractiveNode> {
        let row = self.active_row()?;
        let field = row.fields.get(self.active_field)?;
        Some(field.as_ref())
    }

    fn active_field_widget_mut(&mut self) -> Option<&mut Box<dyn InteractiveNode>> {
        let active_field = self.active_field;
        let row = self.active_row_mut()?;
        row.fields.get_mut(active_field)
    }

    fn active_field_label(&self) -> &str {
        self.fields
            .get(self.active_field)
            .map(|f| f.label.as_str())
            .unwrap_or("Field")
    }

    fn clamp_cursor(&mut self) {
        self.active_item = list_policy::clamp_index(self.active_item, self.rows.len());
        self.active_field = list_policy::clamp_index(self.active_field, self.fields.len());
    }

    fn completed_items(&self) -> usize {
        if self.finished {
            return self.rows.len();
        }
        self.active_item.min(self.rows.len())
    }

    fn has_work(&self) -> bool {
        !self.rows.is_empty() && !self.fields.is_empty()
    }

    fn item_label(&self, row_idx: usize) -> String {
        let Some(row) = self.rows.get(row_idx) else {
            return String::new();
        };
        if let Some(path) = &self.item_label_path
            && let Some(nested) = row.item.get_path(path)
        {
            return display_scalar_or_json(nested);
        }
        display_scalar_or_json(&row.item)
    }

    fn header_line(&self) -> String {
        if self.rows.is_empty() {
            return "configuring [0 of 0]: no items".to_string();
        }
        let total = self.rows.len();
        let index = self.active_item.saturating_add(1).min(total);
        let item = self.item_label(self.active_item);
        self.header_template
            .replace("{index}", index.to_string().as_str())
            .replace("{total}", total.to_string().as_str())
            .replace("{item}", item.as_str())
    }

    fn progress_line(&self) -> Option<String> {
        if !self.show_progress {
            return None;
        }
        let total = self.rows.len();
        let completed = self.completed_items().min(total);
        Some(format!("progress: {completed}/{total} completed"))
    }

    fn build_rows_value_for_count(&self, count: usize) -> Value {
        let limit = count.min(self.rows.len());
        let mut rows = Vec::<Value>::with_capacity(limit);
        for row in self.rows.iter().take(limit) {
            let mut map = IndexMap::<String, Value>::new();
            map.insert("item".to_string(), row.item.clone());
            for (idx, field) in self.fields.iter().enumerate() {
                let value = row
                    .fields
                    .get(idx)
                    .and_then(|widget| widget.value())
                    .unwrap_or(Value::None);
                map.insert(field.key.clone(), value);
            }
            rows.push(Value::Object(map));
        }
        Value::List(rows)
    }

    fn build_rows_value(&self) -> Value {
        self.build_rows_value_for_count(self.rows.len())
    }

    fn build_committed_rows_value(&self) -> Value {
        self.build_rows_value_for_count(self.completed_items())
    }

    fn apply_rows_seed(&mut self, rows_seed: &[Value]) {
        for (row_idx, row_seed) in rows_seed.iter().enumerate() {
            let Some(row) = self.rows.get_mut(row_idx) else {
                break;
            };
            for (field_idx, field) in self.fields.iter().enumerate() {
                if let Some(seed) = seed_value_from_record(
                    Some(row_seed),
                    field_idx,
                    field.key.as_str(),
                    field.label.as_str(),
                ) && let Some(widget) = row.fields.get_mut(field_idx)
                {
                    widget.set_value(seed);
                }
            }
        }
    }

    fn set_rows_value(&mut self, rows: &[Value]) {
        let items = rows.iter().map(extract_item_from_row).collect::<Vec<_>>();
        self.set_items(items);
        self.apply_rows_seed(rows);
    }
}

impl LeafComponent for Repeater {}

fn unique_field_key(fields: &[RepeaterFieldDef], requested: &str) -> String {
    let base = normalize_ascii_key(requested, "field");
    if !fields.iter().any(|field| field.key == base) {
        return base;
    }
    let mut idx = 2usize;
    loop {
        let next = format!("{base}_{idx}");
        if !fields.iter().any(|field| field.key == next) {
            return next;
        }
        idx = idx.saturating_add(1);
    }
}

fn looks_like_rows_list(rows: &[Value], fields: &[RepeaterFieldDef]) -> bool {
    rows.iter().any(|row| match row {
        Value::Object(map) => {
            map.contains_key("item")
                || fields
                    .iter()
                    .any(|field| map.contains_key(field.key.as_str()))
        }
        _ => false,
    })
}

fn extract_item_from_row(row: &Value) -> Value {
    match row {
        Value::Object(map) => map.get("item").cloned().unwrap_or(Value::None),
        _ => Value::None,
    }
}

fn display_scalar_or_json(value: &Value) -> String {
    if let Some(scalar) = value.to_text_scalar() {
        return scalar;
    }
    match value {
        Value::None => "null".to_string(),
        Value::List(_) | Value::Object(_) => {
            let json = value.to_json();
            truncate_text(json.as_str(), 40)
        }
        _ => String::new(),
    }
}

fn truncate_text(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let mut out = input
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}
