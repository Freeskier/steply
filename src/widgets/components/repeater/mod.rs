use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::{ValueChange, WidgetAction};
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionMenu, CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult,
    Interactive, InteractiveNode, RenderContext, TextAction, ValidationMode,
};

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
        let key = normalize_key(label.as_str());
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
        if self.rows.is_empty() {
            self.active_item = 0;
        } else {
            self.active_item = self.active_item.min(self.rows.len().saturating_sub(1));
        }

        if self.fields.is_empty() {
            self.active_field = 0;
        } else {
            self.active_field = self.active_field.min(self.fields.len().saturating_sub(1));
        }
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
        if let Some(path) = &self.item_label_path {
            if let Some(nested) = row.item.get_path(path) {
                return display_scalar_or_json(nested);
            }
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

    fn build_rows_value(&self) -> Value {
        let mut rows = Vec::<Value>::with_capacity(self.rows.len());
        for row in &self.rows {
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

    fn build_value_object(&self) -> Value {
        let items = self.rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>();
        let mut map = IndexMap::<String, Value>::new();
        map.insert("items".to_string(), Value::List(items));
        map.insert("rows".to_string(), self.build_rows_value());
        Value::Object(map)
    }

    fn apply_rows_seed(&mut self, rows_seed: &[Value]) {
        for (row_idx, row_seed) in rows_seed.iter().enumerate() {
            let Some(row) = self.rows.get_mut(row_idx) else {
                break;
            };
            for (field_idx, field) in self.fields.iter().enumerate() {
                if let Some(seed) = seed_value(row_seed, field_idx, field.key.as_str(), field.label.as_str())
                    && let Some(widget) = row.fields.get_mut(field_idx)
                {
                    widget.set_value(seed);
                }
            }
        }
    }

    fn set_value_object(&mut self, map: &IndexMap<String, Value>) {
        let rows_seed = map.get("rows").and_then(Value::as_list).map(|v| v.to_vec());
        let items = match map.get("items") {
            Some(Value::List(list)) => list.clone(),
            _ => rows_seed
                .as_ref()
                .map(|rows| rows.iter().map(extract_item_from_row).collect::<Vec<_>>())
                .unwrap_or_default(),
        };

        self.set_items(items);
        if let Some(rows) = rows_seed {
            self.apply_rows_seed(rows.as_slice());
        }
    }

    fn child_context(&self, ctx: &RenderContext, focused_child_id: Option<String>) -> RenderContext {
        let mut completion_menus = HashMap::<String, CompletionMenu>::new();
        if let Some(child_id) = focused_child_id.as_deref()
            && let Some(menu) = ctx.completion_menus.get(self.base.id())
        {
            completion_menus.insert(child_id.to_string(), menu.clone());
        }

        RenderContext {
            focused_id: focused_child_id,
            terminal_size: ctx.terminal_size,
            visible_errors: HashMap::new(),
            invalid_hidden: HashSet::new(),
            completion_menus,
        }
    }

    fn child_draw_line(
        &self,
        ctx: &RenderContext,
        row_idx: usize,
        field_idx: usize,
        focused: bool,
    ) -> SpanLine {
        let Some(row) = self.rows.get(row_idx) else {
            return vec![Span::new("").no_wrap()];
        };
        let Some(widget) = row.fields.get(field_idx) else {
            return vec![Span::new("").no_wrap()];
        };

        let focused_id = if focused {
            Some(widget.id().to_string())
        } else {
            None
        };
        let child_ctx = self.child_context(ctx, focused_id);
        widget
            .draw(&child_ctx)
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()])
    }

    fn line_prefix_rows(&self) -> usize {
        let mut rows = 0usize;
        if self.show_label && !self.base.label().is_empty() {
            rows += 1;
        }
        rows += 1; // header
        if self.progress_line().is_some() {
            rows += 1;
        }
        rows
    }

    fn draw_single_field(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        let field_label = self.active_field_label();
        let marker_style = if focused {
            Style::new().color(Color::Cyan).bold()
        } else {
            Style::new().color(Color::DarkGrey)
        };
        let mut line = vec![
            Span::styled("❯ ", marker_style).no_wrap(),
            Span::styled(format!("{field_label}: "), Style::new().bold()).no_wrap(),
        ];
        line.extend(self.child_draw_line(ctx, self.active_item, self.active_field, focused));
        lines.push(line);
        lines
    }

    fn draw_stacked_fields(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        let mut lines = Vec::<SpanLine>::new();
        for (field_idx, field) in self.fields.iter().enumerate() {
            let is_active = field_idx == self.active_field;
            let marker = if is_active { "❯ " } else { "  " };
            let marker_style = if is_active && focused {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::new().color(Color::DarkGrey)
            };
            let label_style = if is_active && focused {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::new().bold()
            };
            let mut line = vec![
                Span::styled(marker, marker_style).no_wrap(),
                Span::styled(format!("{}: ", field.label), label_style).no_wrap(),
            ];
            line.extend(self.child_draw_line(
                ctx,
                self.active_item,
                field_idx,
                focused && is_active,
            ));
            lines.push(line);
        }
        lines
    }

    fn draw_empty_state(&self) -> Vec<SpanLine> {
        if self.rows.is_empty() {
            return vec![vec![
                Span::styled("No items to configure.", Style::new().color(Color::DarkGrey)).no_wrap(),
            ]];
        }
        if self.fields.is_empty() {
            return vec![vec![
                Span::styled("No repeater fields configured.", Style::new().color(Color::DarkGrey))
                    .no_wrap(),
            ]];
        }
        vec![]
    }

    fn process_child_result(&mut self, mut result: InteractionResult) -> InteractionResult {
        let mut should_advance = false;
        result.actions.retain(|action| match action {
            WidgetAction::InputDone => {
                should_advance = true;
                false
            }
            _ => true,
        });

        if should_advance {
            result.merge(self.advance_cursor_and_submit_if_done());
        }
        if result.handled {
            result.request_render = true;
        }
        result
    }

    fn advance_cursor_and_submit_if_done(&mut self) -> InteractionResult {
        if !self.has_work() {
            return self.submit_or_done();
        }

        if self.finished {
            return self.submit_or_done();
        }

        if self.active_field + 1 < self.fields.len() {
            self.active_field += 1;
            return InteractionResult::handled();
        }

        if self.active_item + 1 < self.rows.len() {
            self.active_item += 1;
            self.active_field = 0;
            return InteractionResult::handled();
        }

        self.finished = true;
        self.submit_or_done()
    }

    fn retreat_cursor(&mut self) -> InteractionResult {
        if !self.has_work() {
            return InteractionResult::ignored();
        }

        if self.finished {
            self.finished = false;
            return InteractionResult::handled();
        }

        if self.active_field > 0 {
            self.active_field -= 1;
            return InteractionResult::handled();
        }

        if self.active_item > 0 {
            self.active_item -= 1;
            self.active_field = self.fields.len().saturating_sub(1);
            return InteractionResult::handled();
        }

        InteractionResult::ignored()
    }

    fn submit_or_done(&self) -> InteractionResult {
        if let Some(target) = &self.submit_target {
            let mut result = InteractionResult::with_action(WidgetAction::ValueChanged {
                change: ValueChange::with_target(target.clone(), self.build_value_object()),
            });
            result.actions.push(WidgetAction::InputDone);
            return result;
        }
        InteractionResult::input_done()
    }

    fn handle_group_key(&mut self, key: KeyEvent) -> InteractionResult {
        if !self.has_work() {
            return match key.code {
                KeyCode::Enter => self.submit_or_done(),
                _ => InteractionResult::ignored(),
            };
        }

        if let Some(widget) = self.active_field_widget_mut() {
            let result = widget.on_key(key);
            if result.handled {
                return self.process_child_result(result);
            }
        }

        match key.code {
            KeyCode::Enter | KeyCode::Tab => self.advance_cursor_and_submit_if_done(),
            KeyCode::BackTab => self.retreat_cursor(),
            _ => InteractionResult::ignored(),
        }
    }
}

impl Component for Repeater {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl Drawable for Repeater {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let mut lines = Vec::<SpanLine>::new();

        if self.show_label && !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        lines.push(vec![
            Span::styled(self.header_line(), Style::new().color(Color::Yellow).bold()).no_wrap(),
        ]);

        if let Some(progress) = self.progress_line() {
            lines.push(vec![Span::styled(progress, Style::new().color(Color::DarkGrey)).no_wrap()]);
        }

        let mut body = self.draw_empty_state();
        if body.is_empty() {
            body = match self.layout {
                RepeaterLayout::SingleField => self.draw_single_field(ctx, focused),
                RepeaterLayout::Stacked => self.draw_stacked_fields(ctx, focused),
            };
        }
        lines.extend(body);

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

impl Interactive for Repeater {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        self.handle_group_key(key)
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let result = {
            let Some(widget) = self.active_field_widget_mut() else {
                return InteractionResult::ignored();
            };
            widget.on_text_action(action)
        };
        if !result.handled {
            return InteractionResult::ignored();
        }
        self.process_child_result(result)
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.active_field_widget_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if !self.has_work() {
            return None;
        }
        let local = self.active_field_widget()?.cursor_pos()?;
        let base = self.line_prefix_rows();
        let row = match self.layout {
            RepeaterLayout::SingleField => base,
            RepeaterLayout::Stacked => base.saturating_add(self.active_field),
        };
        Some(CursorPos {
            col: local
                .col
                .saturating_add(self.active_field_label().len() as u16 + 4),
            row: row as u16 + local.row,
        })
    }

    fn value(&self) -> Option<Value> {
        Some(self.build_value_object())
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::None => self.set_items(Vec::new()),
            Value::List(items) => self.set_items(items),
            Value::Object(map) => self.set_value_object(&map),
            scalar => self.set_items(vec![scalar]),
        }
        self.finished = false;
        self.clamp_cursor();
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (field_idx, widget) in row.fields.iter().enumerate() {
                if let Err(err) = widget.validate(mode) {
                    let field_label = self
                        .fields
                        .get(field_idx)
                        .map(|f| f.label.as_str())
                        .unwrap_or("field");
                    let item = self.item_label(row_idx);
                    return Err(format!("item {} [{}], {}: {}", row_idx + 1, item, field_label, err));
                }
            }
        }
        Ok(())
    }
}

fn normalize_key(input: &str) -> String {
    let mut key = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
            prev_underscore = false;
            continue;
        }
        if !prev_underscore && !key.is_empty() {
            key.push('_');
            prev_underscore = true;
        }
    }
    while key.ends_with('_') {
        key.pop();
    }
    if key.is_empty() {
        "field".to_string()
    } else {
        key
    }
}

fn unique_field_key(fields: &[RepeaterFieldDef], requested: &str) -> String {
    let base = normalize_key(requested);
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

fn seed_value(seed: &Value, field_idx: usize, key: &str, label: &str) -> Option<Value> {
    match seed {
        Value::Object(map) => map.get(key).cloned().or_else(|| map.get(label).cloned()),
        Value::List(list) => list.get(field_idx).cloned(),
        Value::None => None,
        scalar if field_idx == 0 => Some(scalar.clone()),
        _ => None,
    }
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
    let mut out = input.chars().take(max_chars.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::{Repeater, RepeaterLayout};
    use crate::core::value::Value;
    use crate::runtime::event::WidgetAction;
    use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
    use crate::widgets::inputs::text::TextInput;
    use crate::widgets::traits::Interactive;
    use indexmap::IndexMap;

    #[test]
    fn set_value_list_becomes_items() {
        let mut repeater = Repeater::new("r", "R")
            .with_layout(RepeaterLayout::SingleField)
            .field_auto("Path", TextInput::new);
        repeater.set_value(Value::List(vec![
            Value::Text("Kasia".to_string()),
            Value::Text("Jas".to_string()),
        ]));

        let value = repeater.value().expect("value");
        let Value::Object(map) = value else {
            panic!("expected object");
        };
        let Value::List(items) = map.get("items").cloned().unwrap_or(Value::None) else {
            panic!("expected items list");
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn value_object_seeds_row_fields() {
        let mut row = IndexMap::<String, Value>::new();
        row.insert("item".to_string(), Value::Text("Kasia".to_string()));
        row.insert("path".to_string(), Value::Text("/tmp/out".to_string()));

        let mut root = IndexMap::<String, Value>::new();
        root.insert(
            "items".to_string(),
            Value::List(vec![Value::Text("Kasia".to_string())]),
        );
        root.insert("rows".to_string(), Value::List(vec![Value::Object(row)]));

        let mut repeater = Repeater::new("r", "R").field_auto("Path", TextInput::new);
        repeater.set_value(Value::Object(root));

        let value = repeater.value().expect("value");
        let Value::Object(map) = value else {
            panic!("expected object");
        };
        let Value::List(rows) = map.get("rows").cloned().unwrap_or(Value::None) else {
            panic!("expected rows");
        };
        let Some(Value::Object(first)) = rows.first() else {
            panic!("expected first row object");
        };
        assert_eq!(
            first.get("path").and_then(Value::as_text),
            Some("/tmp/out")
        );
    }

    #[test]
    fn submit_with_target_emits_value_change_and_input_done() {
        let mut repeater = Repeater::new("r", "R")
            .with_items(vec![Value::Text("Kasia".into())])
            .field_auto("Path", TextInput::new)
            .with_submit_target("rep_out");

        let result = repeater.on_key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        });

        assert!(result.handled);
        assert_eq!(result.actions.len(), 2);
        assert!(matches!(result.actions[0], WidgetAction::ValueChanged { .. }));
        assert!(matches!(result.actions[1], WidgetAction::InputDone));
    }
}
