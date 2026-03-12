use indexmap::IndexMap;

use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValuePath, ValueTarget};
use crate::runtime::event::{ValueChange, WidgetAction};
use crate::state::store::ValueStore;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, PointerEvent};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{LeafComponent, Node};
use crate::widgets::shared::binding::ReadBinding;
use crate::widgets::shared::render_ctx::child_context_for;
use crate::widgets::shared::validation::decorate_component_validation;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, TextAction, ValidationMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeaterIterationMode {
    #[default]
    Fixed,
    Append,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterLayout {
    SingleField,
    Stacked,
}

pub struct Repeater {
    base: WidgetBase,
    widgets: Vec<Node>,
    active_widget: usize,
    items_binding: Option<ReadBinding>,
    count_binding: Option<ReadBinding>,
    items: Vec<Value>,
    explicit_count: Option<usize>,
    rows: Vec<Value>,
    current_row: Value,
    active_index: usize,
    layout: RepeaterLayout,
    mode: RepeaterIterationMode,
    show_label: bool,
    show_progress: bool,
    header_template: String,
    item_label_path: Option<ValuePath>,
    finished: bool,
    store_snapshot: ValueStore,
    last_items_value: Option<Option<Value>>,
    last_count_value: Option<Option<Value>>,
}

impl Repeater {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            widgets: Vec::new(),
            active_widget: 0,
            items_binding: None,
            count_binding: None,
            items: Vec::new(),
            explicit_count: None,
            rows: Vec::new(),
            current_row: empty_row(),
            active_index: 0,
            layout: RepeaterLayout::SingleField,
            mode: RepeaterIterationMode::Fixed,
            show_label: true,
            show_progress: true,
            header_template: "configuring [{index} of {count}]".to_string(),
            item_label_path: None,
            finished: false,
            store_snapshot: ValueStore::new(),
            last_items_value: None,
            last_count_value: None,
        }
    }

    pub fn with_layout(mut self, layout: RepeaterLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_mode(mut self, mode: RepeaterIterationMode) -> Self {
        self.mode = mode;
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

    pub fn with_items_binding(mut self, binding: ReadBinding) -> Self {
        self.items_binding = Some(binding);
        self
    }

    pub fn with_items(mut self, items: Vec<Value>) -> Self {
        self.items = items;
        self
    }

    pub fn with_count_binding(mut self, binding: ReadBinding) -> Self {
        self.count_binding = Some(binding);
        self
    }

    pub fn with_widget(mut self, widget: Node) -> Self {
        self.widgets.push(widget);
        self.clamp_cursor();
        self
    }

    fn child_context(
        &self,
        ctx: &RenderContext,
        focused_child_id: Option<String>,
    ) -> RenderContext {
        child_context_for(self.base.id(), ctx, focused_child_id)
    }

    fn active_widget_ref(&self) -> Option<&Node> {
        self.widgets.get(self.active_widget)
    }

    fn active_widget_mut(&mut self) -> Option<&mut Node> {
        self.widgets.get_mut(self.active_widget)
    }

    fn clamp_cursor(&mut self) {
        if self.widgets.is_empty() {
            self.active_widget = 0;
        } else {
            self.active_widget = self.active_widget.min(self.widgets.len().saturating_sub(1));
        }
    }

    fn total_count(&self) -> usize {
        match self.mode {
            RepeaterIterationMode::Fixed => self.explicit_count.unwrap_or(self.items.len()),
            RepeaterIterationMode::Append => {
                self.explicit_count.unwrap_or_else(|| self.rows.len() + 1)
            }
        }
    }

    fn current_item(&self) -> Option<&Value> {
        self.items.get(self.active_index)
    }

    fn header_line(&self) -> String {
        let index = self.active_index.saturating_add(1);
        let count = self.total_count();
        let item = self
            .current_item()
            .map(|value| self.item_label(value))
            .unwrap_or_default();
        self.header_template
            .replace("{index}", index.to_string().as_str())
            .replace("{count}", count.to_string().as_str())
            .replace("{item}", item.as_str())
    }

    fn item_label(&self, value: &Value) -> String {
        if let Some(path) = &self.item_label_path
            && let Some(nested) = value.get_path(path)
        {
            return display_scalar_or_json(nested);
        }
        display_scalar_or_json(value)
    }

    fn progress_line(&self) -> Option<String> {
        if !self.show_progress {
            return None;
        }
        let count = self.total_count();
        let completed = self.rows.len().min(count);
        Some(format!("progress: {completed}/{count} completed"))
    }

    fn scoped_store(&self) -> ValueStore {
        let mut scoped = ValueStore::new();
        for (key, value) in self.store_snapshot.iter() {
            let _ = scoped.set(key.to_string(), value.clone());
        }
        let _ = scoped.set("_row", self.current_row.clone());
        if let Some(item) = self.current_item() {
            let _ = scoped.set("_item", item.clone());
        } else {
            let _ = scoped.set("_item", Value::None);
        }
        let _ = scoped.set("_index", Value::Number(self.active_index as f64));
        let _ = scoped.set("_count", Value::Number(self.total_count() as f64));
        scoped
    }

    fn sync_widgets_from_scope(&mut self) -> bool {
        let scoped = self.scoped_store();
        let mut changed = false;
        for widget in &mut self.widgets {
            changed |= widget.sync_from_store(&scoped);
        }
        changed
    }

    fn refresh_sources(&mut self, store: &ValueStore) -> bool {
        self.store_snapshot = clone_store(store);
        let mut changed = false;

        let next_items = self
            .items_binding
            .as_ref()
            .and_then(|binding| binding.resolve(store));
        if self.last_items_value.as_ref() != Some(&next_items) {
            self.last_items_value = Some(next_items.clone());
            self.items = match next_items {
                Some(Value::List(items)) => items,
                Some(Value::None) | None => Vec::new(),
                Some(value) => vec![value],
            };
            changed = true;
        }

        let next_count = self
            .count_binding
            .as_ref()
            .and_then(|binding| binding.resolve(store));
        if self.last_count_value.as_ref() != Some(&next_count) {
            self.last_count_value = Some(next_count.clone());
            self.explicit_count = next_count.as_ref().and_then(read_count_value);
            changed = true;
        }

        if changed {
            self.active_index = self.active_index.min(self.max_active_index());
            self.load_current_row();
        }

        changed
    }

    fn max_active_index(&self) -> usize {
        self.total_count().saturating_sub(1)
    }

    fn load_current_row(&mut self) {
        self.current_row = self
            .rows
            .get(self.active_index)
            .cloned()
            .unwrap_or_else(empty_row);
    }

    fn commit_current_row(&mut self) {
        if self.active_index < self.rows.len() {
            self.rows[self.active_index] = self.current_row.clone();
        } else {
            self.rows.push(self.current_row.clone());
        }
    }

    fn next_iteration(&mut self) -> bool {
        self.commit_current_row();
        self.active_widget = 0;
        match self.mode {
            RepeaterIterationMode::Fixed => {
                if self.active_index + 1 >= self.total_count() {
                    self.finished = true;
                    return false;
                }
                self.active_index += 1;
            }
            RepeaterIterationMode::Append => {
                if self.active_index + 1 >= self.total_count() {
                    self.finished = true;
                    return false;
                }
                self.active_index += 1;
            }
        }
        self.current_row = self
            .rows
            .get(self.active_index)
            .cloned()
            .unwrap_or_else(empty_row);
        let _ = self.sync_widgets_from_scope();
        true
    }

    fn previous_iteration(&mut self) -> bool {
        if self.active_index == 0 {
            return false;
        }
        self.active_index -= 1;
        self.active_widget = self.widgets.len().saturating_sub(1);
        self.finished = false;
        self.load_current_row();
        let _ = self.sync_widgets_from_scope();
        true
    }

    fn draw_single_widget(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        let Some(widget) = self.active_widget_ref() else {
            return vec![empty_line("No repeater widgets configured.")];
        };
        let focused_id = focused.then(|| widget.id().to_string());
        let child_ctx = self.child_context(ctx, focused_id);
        let mut out = widget.draw(&child_ctx).lines;
        if out.is_empty() {
            out.push(vec![Span::new(String::new()).no_wrap()]);
        }
        if let Some(first) = out.first_mut() {
            first.insert(
                0,
                Span::styled(
                    "❯ ",
                    if focused {
                        Style::new().color(Color::Cyan).bold()
                    } else {
                        Style::new().color(Color::DarkGrey)
                    },
                )
                .no_wrap(),
            );
        }
        out
    }

    fn draw_stacked_widgets(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        if self.widgets.is_empty() {
            return vec![empty_line("No repeater widgets configured.")];
        }

        let mut lines = Vec::new();
        for (index, widget) in self.widgets.iter().enumerate() {
            let is_active = index == self.active_widget;
            let focused_id = (focused && is_active).then(|| widget.id().to_string());
            let child_ctx = self.child_context(ctx, focused_id);
            let mut out = widget.draw(&child_ctx).lines;
            if out.is_empty() {
                out.push(vec![Span::new(String::new()).no_wrap()]);
            }
            if let Some(first) = out.first_mut() {
                first.insert(
                    0,
                    Span::styled(
                        if is_active { "❯ " } else { "  " },
                        if focused && is_active {
                            Style::new().color(Color::Cyan).bold()
                        } else {
                            Style::new().color(Color::DarkGrey)
                        },
                    )
                    .no_wrap(),
                );
            }
            lines.extend(out);
        }
        lines
    }

    fn line_prefix_rows(&self) -> usize {
        let mut rows = 0usize;
        if self.show_label && !self.base.label().is_empty() {
            rows += 1;
        }
        rows += 1;
        if self.progress_line().is_some() {
            rows += 1;
        }
        rows
    }

    fn apply_local_change(&mut self, change: ValueChange) -> bool {
        let Some(target) = localize_row_target(&change.target) else {
            return false;
        };
        let mut store = ValueStore::new();
        let _ = store.set("_row", self.current_row.clone());
        if store.set_target(&target, change.value).is_err() {
            return false;
        }
        self.current_row = store.get("_row").cloned().unwrap_or_else(empty_row);
        true
    }

    fn process_child_result(&mut self, mut result: InteractionResult) -> InteractionResult {
        let mut should_advance = false;
        let mut retained = Vec::with_capacity(result.actions.len());

        for action in result.actions.drain(..) {
            match action {
                WidgetAction::InputDone => {
                    should_advance = true;
                }
                WidgetAction::ValueChanged { change } => {
                    if !self.apply_local_change(change.clone()) {
                        retained.push(WidgetAction::ValueChanged { change });
                    }
                }
                other => retained.push(other),
            }
        }

        result.actions = retained;

        if self.sync_widgets_from_scope() {
            result.handled = true;
            result.request_render = true;
        }

        if should_advance {
            if self.active_widget + 1 < self.widgets.len() {
                self.active_widget += 1;
                result.handled = true;
                result.request_render = true;
            } else if self.next_iteration() {
                result.handled = true;
                result.request_render = true;
            } else {
                result.merge(InteractionResult::input_done());
            }
        }

        result
    }
}

impl LeafComponent for Repeater {}

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
            lines.push(vec![
                Span::styled(progress, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        let body = match self.layout {
            RepeaterLayout::SingleField => self.draw_single_widget(ctx, focused),
            RepeaterLayout::Stacked => self.draw_stacked_widgets(ctx, focused),
        };
        lines.extend(body);

        decorate_component_validation(&mut lines, ctx, self.base.id());
        DrawOutput::with_lines(lines)
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }
        vec![
            HintItem::new("Tab", "next field", HintGroup::Navigation).with_priority(20),
            HintItem::new("Shift+Tab", "previous field", HintGroup::Navigation).with_priority(20),
            HintItem::new("Enter", "commit and next", HintGroup::Action).with_priority(30),
        ]
    }
}

impl Interactive for Repeater {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.finished {
            return match key.code {
                KeyCode::Enter => InteractionResult::input_done(),
                _ => InteractionResult::ignored(),
            };
        }

        if let Some(widget) = self.active_widget_mut() {
            let result = widget.on_key(key);
            if result.handled {
                return self.process_child_result(result);
            }
        }

        match key.code {
            KeyCode::Enter | KeyCode::Tab => {
                self.process_child_result(InteractionResult::input_done())
            }
            KeyCode::BackTab => {
                if self.active_widget > 0 {
                    self.active_widget -= 1;
                    InteractionResult::handled()
                } else if self.previous_iteration() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_pointer(&mut self, _event: PointerEvent) -> InteractionResult {
        InteractionResult::ignored()
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let Some(widget) = self.active_widget_mut() else {
            return InteractionResult::ignored();
        };
        let result = widget.on_text_action(action);
        if result.handled {
            self.process_child_result(result)
        } else {
            InteractionResult::ignored()
        }
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.active_widget_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let widget = self.active_widget_ref()?;
        let local = widget.cursor_pos()?;
        let row_offset = self.line_prefix_rows() as u16;
        Some(CursorPos {
            col: local.col.saturating_add(2),
            row: local.row.saturating_add(row_offset),
        })
    }

    fn value(&self) -> Option<Value> {
        Some(Value::List(self.rows.clone()))
    }

    fn set_value(&mut self, value: Value) {
        self.rows = match value {
            Value::List(rows) => rows,
            Value::Object(map) => match map.get("rows") {
                Some(Value::List(rows)) => rows.clone(),
                _ => Vec::new(),
            },
            Value::None => Vec::new(),
            scalar => vec![scalar],
        };
        self.active_index = self.rows.len().min(self.max_active_index());
        self.load_current_row();
        let _ = self.sync_widgets_from_scope();
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        let changed = self.refresh_sources(store);
        let child_changed = self.sync_widgets_from_scope();
        changed || child_changed
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for widget in &self.widgets {
            widget.validate(mode)?;
        }
        Ok(())
    }

    fn task_specs(&self) -> Vec<crate::task::TaskSpec> {
        self.widgets.iter().flat_map(Node::task_specs).collect()
    }

    fn task_subscriptions(&self) -> Vec<crate::task::TaskSubscription> {
        self.widgets
            .iter()
            .flat_map(Node::task_subscriptions)
            .collect()
    }
}

fn clone_store(store: &ValueStore) -> ValueStore {
    let mut out = ValueStore::new();
    for (key, value) in store.iter() {
        let _ = out.set(key.to_string(), value.clone());
    }
    out
}

fn read_count_value(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => Some((*number).max(0.0) as usize),
        Value::Text(text) => text.trim().parse::<usize>().ok(),
        Value::List(items) => Some(items.len()),
        _ => None,
    }
}

fn empty_row() -> Value {
    Value::Object(IndexMap::new())
}

fn empty_line(text: &str) -> SpanLine {
    vec![Span::styled(text.to_string(), Style::new().color(Color::DarkGrey)).no_wrap()]
}

fn display_scalar_or_json(value: &Value) -> String {
    value.to_text_scalar().unwrap_or_else(|| value.to_json())
}

fn localize_row_target(target: &ValueTarget) -> Option<ValueTarget> {
    match target {
        ValueTarget::Node(root) if !root.as_str().starts_with('_') => Some(ValueTarget::path(
            "_row".to_string(),
            ValuePath::new(vec![PathSegment::Key(root.to_string())]),
        )),
        ValueTarget::Path { root, path } if root.as_str() == "_row" => {
            Some(ValueTarget::path("_row".to_string(), path.clone()))
        }
        _ => None,
    }
}
