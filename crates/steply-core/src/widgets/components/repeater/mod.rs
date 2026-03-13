use indexmap::IndexMap;

use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValuePath, ValueTarget};
use crate::runtime::event::{ValueChange, WidgetAction};
use crate::state::step::StepCondition;
use crate::state::store::ValueStore;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, PointerEvent};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::ui::text::text_display_width;
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{LeafComponent, Node};
use crate::widgets::shared::binding::ReadBinding;
use crate::widgets::shared::render_ctx::child_context_for;
use crate::widgets::shared::validation::decorate_component_validation;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, StoreSyncPolicy, TextAction, ValidationMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterEntryMode {
    Progressive,
    Full,
}

pub struct Repeater {
    base: WidgetBase,
    widgets: Vec<Node>,
    active_widget: usize,
    iterate_binding: Option<ReadBinding>,
    items: Vec<Value>,
    explicit_count: usize,
    rows: Vec<Value>,
    current_row: Value,
    active_index: usize,
    entry_mode: RepeaterEntryMode,
    show_label: bool,
    show_progress: bool,
    header_binding: Option<ReadBinding>,
    item_label_path: Option<ValuePath>,
    finish_condition: Option<StepCondition>,
    finished: bool,
    awaiting_finish_resolution: bool,
    pending_finish_done: bool,
    store_snapshot: ValueStore,
    last_iterate_value: Option<Option<Value>>,
    finish_condition_snapshot: Vec<(String, Option<Value>)>,
}

impl Repeater {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            widgets: Vec::new(),
            active_widget: 0,
            iterate_binding: None,
            items: Vec::new(),
            explicit_count: 0,
            rows: Vec::new(),
            current_row: empty_row(),
            active_index: 0,
            entry_mode: RepeaterEntryMode::Progressive,
            show_label: true,
            show_progress: true,
            header_binding: Some(ReadBinding::Template(
                "configuring [{{_position}} of {{_count}}]".to_string(),
            )),
            item_label_path: None,
            finish_condition: None,
            finished: false,
            awaiting_finish_resolution: false,
            pending_finish_done: false,
            store_snapshot: ValueStore::new(),
            last_iterate_value: None,
            finish_condition_snapshot: Vec::new(),
        }
    }

    pub fn with_entry_mode(mut self, entry_mode: RepeaterEntryMode) -> Self {
        self.entry_mode = entry_mode;
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

    pub fn with_header_binding(mut self, binding: ReadBinding) -> Self {
        self.header_binding = Some(binding);
        self
    }

    pub fn with_header_template(mut self, template: impl Into<String>) -> Self {
        self.header_binding = Some(ReadBinding::Template(template.into()));
        self
    }

    pub fn with_item_label_path(mut self, path: ValuePath) -> Self {
        self.item_label_path = Some(path);
        self
    }

    pub fn with_iterate_binding(mut self, binding: ReadBinding) -> Self {
        self.iterate_binding = Some(binding);
        self
    }

    pub fn with_finish_condition(mut self, condition: StepCondition) -> Self {
        self.finish_condition = Some(condition);
        self
    }

    pub fn with_items(mut self, items: Vec<Value>) -> Self {
        self.explicit_count = items.len();
        self.items = items;
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.items.clear();
        self.explicit_count = count;
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
        label_offset: u16,
    ) -> RenderContext {
        child_context_for(self.base.id(), ctx, focused_child_id).with_terminal_width(
            ctx.terminal_size
                .width
                .saturating_sub(2)
                .saturating_sub(label_offset),
        )
    }

    fn child_label_prefix(&self, widget: &Node, focused: bool) -> (Option<SpanLine>, u16) {
        let Node::Input(widget) = widget else {
            return (None, 0);
        };

        let label = widget.label();
        if label.is_empty() {
            return (None, 0);
        }

        let style = if focused {
            Style::new().color(Color::White)
        } else {
            Style::default()
        };
        (
            Some(vec![Span::styled(format!("{label}: "), style).no_wrap()]),
            text_display_width(label)
                .saturating_add(2)
                .min(u16::MAX as usize) as u16,
        )
    }

    fn draw_child_widget(
        &self,
        widget: &Node,
        ctx: &RenderContext,
        focused_id: Option<String>,
    ) -> (Vec<SpanLine>, u16) {
        let is_focused = focused_id.as_deref().is_some_and(|id| id == widget.id());
        let (label_prefix, label_offset) = self.child_label_prefix(widget, is_focused);
        let child_ctx = self.child_context(ctx, focused_id, label_offset);
        let mut out = widget.draw(&child_ctx).lines;
        if let Some(prefix) = label_prefix
            && let Some(first) = out.first_mut()
        {
            let mut new_first = prefix;
            new_first.append(first);
            *first = new_first;
        }
        (out, label_offset)
    }

    fn active_widget_ref(&self) -> Option<&Node> {
        self.widgets.get(self.active_widget)
    }

    fn active_widget_mut(&mut self) -> Option<&mut Node> {
        self.widgets.get_mut(self.active_widget)
    }

    fn clamp_cursor(&mut self) {
        self.active_widget = self
            .focusable_widget_index_from(self.active_widget, true)
            .or_else(|| self.first_focusable_widget_index())
            .unwrap_or(0);
    }

    fn is_focusable_widget(widget: &Node) -> bool {
        matches!(widget.focus_mode(), FocusMode::Leaf | FocusMode::Group)
    }

    fn first_focusable_widget_index(&self) -> Option<usize> {
        self.widgets.iter().position(Self::is_focusable_widget)
    }

    fn last_focusable_widget_index(&self) -> Option<usize> {
        self.widgets.iter().rposition(Self::is_focusable_widget)
    }

    fn next_focusable_widget_index(&self, current: usize) -> Option<usize> {
        self.widgets
            .iter()
            .enumerate()
            .skip(current.saturating_add(1))
            .find_map(|(index, widget)| Self::is_focusable_widget(widget).then_some(index))
    }

    fn previous_focusable_widget_index(&self, current: usize) -> Option<usize> {
        self.widgets
            .iter()
            .enumerate()
            .take(current)
            .rev()
            .find_map(|(index, widget)| Self::is_focusable_widget(widget).then_some(index))
    }

    fn focusable_widget_index_from(&self, start: usize, include_start: bool) -> Option<usize> {
        self.widgets
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, widget)| {
                if (!include_start && index == start) || !Self::is_focusable_widget(widget) {
                    None
                } else {
                    Some(index)
                }
            })
    }

    fn total_count(&self) -> usize {
        self.explicit_count
    }

    fn current_item(&self) -> Option<&Value> {
        self.items.get(self.active_index)
    }

    fn header_line(&self) -> Option<String> {
        let binding = self.header_binding.as_ref()?;
        let scoped = self.scoped_store();
        let value = binding.resolve(&scoped)?;
        let text = display_scalar_or_json(&value);
        (!text.trim().is_empty()).then_some(text)
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
            let _ = scoped.set("_item_label", Value::Text(self.current_item_label(item)));
        } else {
            let _ = scoped.set("_item", Value::None);
            let _ = scoped.set("_item_label", Value::Text(String::new()));
        }
        let _ = scoped.set("_index", Value::Number(self.active_index as f64));
        let _ = scoped.set(
            "_position",
            Value::Number(self.active_index.saturating_add(1) as f64),
        );
        let _ = scoped.set("_count", Value::Number(self.total_count() as f64));
        scoped
    }

    fn current_item_label(&self, value: &Value) -> String {
        if let Some(path) = &self.item_label_path
            && let Some(nested) = value.get_path(path)
        {
            return display_scalar_or_json(nested);
        }
        display_scalar_or_json(value)
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
        let next_iterate = self
            .iterate_binding
            .as_ref()
            .and_then(|binding| binding.resolve(store));
        if self.last_iterate_value.as_ref() == Some(&next_iterate) {
            return false;
        }

        self.last_iterate_value = Some(next_iterate.clone());
        let (items, explicit_count) = resolved_iterate_state(next_iterate.as_ref());
        self.items = items;
        self.explicit_count = explicit_count;
        self.active_index = self.active_index.min(self.max_active_index());
        self.finished = false;
        self.load_current_row();
        true
    }

    fn max_active_index(&self) -> usize {
        self.total_count().saturating_sub(1)
    }

    fn load_current_row(&mut self) {
        if self.total_count() == 0 {
            self.current_row = empty_row();
            return;
        }
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
        if self.widgets.is_empty() || self.total_count() == 0 {
            self.finished = true;
            return false;
        }
        self.commit_current_row();
        self.active_widget = self.first_focusable_widget_index().unwrap_or(0);
        if self.active_index + 1 >= self.total_count() {
            self.finished = true;
            return false;
        }
        self.active_index += 1;
        self.current_row = self
            .rows
            .get(self.active_index)
            .cloned()
            .unwrap_or_else(empty_row);
        let _ = self.sync_widgets_from_scope();
        true
    }

    fn advance_after_committed_row(&mut self) -> bool {
        if self.widgets.is_empty() || self.total_count() == 0 {
            self.finished = true;
            return false;
        }
        self.active_widget = self.first_focusable_widget_index().unwrap_or(0);
        if self.active_index + 1 >= self.total_count() {
            self.finished = true;
            return false;
        }
        self.active_index += 1;
        self.finished = false;
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
        self.active_widget = self
            .last_focusable_widget_index()
            .unwrap_or_else(|| self.widgets.len().saturating_sub(1));
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
        let (mut out, _) = self.draw_child_widget(widget, ctx, focused_id);
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
            if let Some(next) = first.get_mut(1) {
                next.no_wrap_join_prev = true;
            }
        }
        out
    }

    fn draw_full_widgets(&self, ctx: &RenderContext, focused: bool) -> Vec<SpanLine> {
        if self.widgets.is_empty() {
            return vec![empty_line("No repeater widgets configured.")];
        }

        let mut lines = Vec::new();
        for (index, widget) in self.widgets.iter().enumerate() {
            let is_active = index == self.active_widget;
            let focused_id = (focused && is_active).then(|| widget.id().to_string());
            let (mut out, _) = self.draw_child_widget(widget, ctx, focused_id);
            if out.is_empty() {
                continue;
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
                if let Some(next) = first.get_mut(1) {
                    next.no_wrap_join_prev = true;
                }
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
        if self.header_line().is_some() {
            rows += 1;
        }
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

    fn capture_active_widget_value(&mut self) -> bool {
        let changes = self
            .active_widget_ref()
            .map(Node::write_changes)
            .unwrap_or_default();
        let mut changed = false;
        for change in changes {
            changed |= self.apply_local_change(change);
        }
        changed
    }

    fn process_child_result(&mut self, mut result: InteractionResult) -> InteractionResult {
        let mut should_advance = false;
        let mut retained = Vec::with_capacity(result.actions.len());

        for action in result.actions.drain(..) {
            match action {
                WidgetAction::InputDone => {
                    should_advance = true;
                }
                WidgetAction::ValueChanged { source, change } => {
                    if !self.apply_local_change(change.clone()) {
                        retained.push(WidgetAction::ValueChanged { source, change });
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
            if self.capture_active_widget_value() {
                result.handled = true;
                result.request_render = true;
            }
            if let Some(next_index) = self.next_focusable_widget_index(self.active_widget) {
                self.active_widget = next_index;
                result.handled = true;
                result.request_render = true;
            } else if self.begin_finish_resolution() {
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

    fn begin_finish_resolution(&mut self) -> bool {
        if self.finish_condition.is_none() {
            return false;
        }
        self.commit_current_row();
        let scoped = self.scoped_store();
        let (fields, immediate_should_finish) = {
            let condition = self
                .finish_condition
                .as_ref()
                .expect("checked finish_condition above");
            let fields = condition.referenced_fields();
            let immediate_should_finish = (fields.is_empty()
                || fields.iter().all(|field| field.starts_with('_')))
            .then(|| condition.evaluate(&scoped));
            (fields, immediate_should_finish)
        };

        if let Some(should_finish) = immediate_should_finish {
            if should_finish {
                self.finished = true;
                self.pending_finish_done = true;
                return true;
            }
            return self.advance_after_committed_row();
        }

        self.awaiting_finish_resolution = true;
        self.pending_finish_done = false;
        self.finish_condition_snapshot = fields
            .into_iter()
            .map(|field| (field.to_string(), scoped.get_selector(field).cloned()))
            .collect();
        true
    }

    fn maybe_resolve_finish_resolution(&mut self) -> bool {
        if !self.awaiting_finish_resolution {
            return false;
        }

        let Some(condition) = self.finish_condition.as_ref() else {
            self.awaiting_finish_resolution = false;
            return false;
        };

        let scoped = self.scoped_store();
        let observed_changed = self
            .finish_condition_snapshot
            .iter()
            .any(|(field, previous)| scoped.get_selector(field.as_str()).cloned() != *previous);
        if !observed_changed {
            return false;
        }

        self.awaiting_finish_resolution = false;
        self.finish_condition_snapshot.clear();

        let should_finish = condition.evaluate(&scoped);
        if should_finish {
            self.finished = true;
            self.pending_finish_done = true;
            return true;
        }

        if self.advance_after_committed_row() {
            return true;
        }

        self.finished = true;
        self.pending_finish_done = true;
        true
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
        if let Some(header) = self.header_line() {
            lines.push(vec![
                Span::styled(header, Style::new().color(Color::Yellow).bold()).no_wrap(),
            ]);
        }

        if let Some(progress) = self.progress_line() {
            lines.push(vec![
                Span::styled(progress, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        let body = match self.entry_mode {
            RepeaterEntryMode::Progressive => self.draw_single_widget(ctx, focused),
            RepeaterEntryMode::Full => self.draw_full_widgets(ctx, focused),
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

    fn store_sync_policy(&self) -> StoreSyncPolicy {
        StoreSyncPolicy::PreserveLocalStateWhileFocused
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.awaiting_finish_resolution {
            return InteractionResult::ignored();
        }

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
                if let Some(previous_index) =
                    self.previous_focusable_widget_index(self.active_widget)
                {
                    self.active_widget = previous_index;
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

    fn on_tick(&mut self) -> InteractionResult {
        if !self.pending_finish_done {
            return InteractionResult::ignored();
        }
        self.pending_finish_done = false;
        InteractionResult::input_done()
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
        self.cursor_pos_with_width(u16::MAX)
    }

    fn cursor_pos_with_width(&self, available_width: u16) -> Option<CursorPos> {
        let widget = self.active_widget_ref()?;
        let (_, label_offset) = self.child_label_prefix(widget, true);
        let local = widget.cursor_pos_with_width(
            available_width
                .saturating_sub(2)
                .saturating_sub(label_offset),
        )?;
        let row_offset = self.line_prefix_rows() as u16;
        Some(CursorPos {
            col: local.col.saturating_add(2).saturating_add(label_offset),
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
        let finish_changed = self.maybe_resolve_finish_resolution();
        changed || child_changed || finish_changed
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

fn resolved_iterate_state(value: Option<&Value>) -> (Vec<Value>, usize) {
    match value {
        Some(Value::List(items)) => (items.clone(), items.len()),
        Some(value) => (Vec::new(), read_count_value(value).unwrap_or(0)),
        None => (Vec::new(), 0),
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

#[cfg(test)]
#[path = "../tests/repeater.rs"]
mod tests;
