use crate::core::store_refs::{
    parse_store_selector, render_template as render_resolved_template, resolve_template_value,
};
use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValuePath, ValueTarget};
use crate::runtime::event::{SystemEvent, ValueChange, WidgetAction};
use crate::state::store::ValueStore;
use crate::task::{TaskSpec, TaskSubscription};
use crate::terminal::{CursorPos, KeyEvent, PointerEvent};
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, OutputNode,
    OverlayMode, OverlayPlacement, PointerRowMap, RenderContext, TextAction, TextEditState,
    ValidationMode,
};
use indexmap::IndexMap;

#[derive(Debug, Clone, Default)]
pub struct StoreBinding {
    pub value: Option<ValueTarget>,
    pub options: Option<ReadBinding>,
    pub reads: Option<ReadBinding>,
    pub writes: Vec<WriteBinding>,
}

impl StoreBinding {
    pub fn is_empty(&self) -> bool {
        self.value.is_none()
            && self.options.is_none()
            && self.reads.is_none()
            && self.writes.is_empty()
    }

    pub fn read_value(&self, store: &ValueStore) -> Option<Value> {
        self.reads.as_ref().and_then(|reads| reads.resolve(store))
    }

    pub fn interactive_read_mode(&self) -> InteractiveReadMode {
        if self.value.is_some() {
            InteractiveReadMode::Controlled
        } else {
            InteractiveReadMode::Seeded
        }
    }

    pub fn write_changes(&self, value: Option<Value>) -> Vec<ValueChange> {
        if self.writes.is_empty() {
            return Vec::new();
        }

        let value = value.unwrap_or(Value::None);
        let scope = build_scope(self.reads.as_ref(), &value);
        self.writes
            .iter()
            .map(|binding| {
                ValueChange::with_target(binding.target.clone(), binding.expr.resolve(&scope))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveReadMode {
    Controlled,
    Seeded,
}

#[derive(Debug, Clone)]
pub enum ReadBinding {
    Selector(ValueTarget),
    Literal(Value),
    Template(String),
    Object(IndexMap<String, ReadBinding>),
    List(Vec<ReadBinding>),
}

impl ReadBinding {
    pub(crate) fn resolve(&self, store: &ValueStore) -> Option<Value> {
        match self {
            Self::Selector(target) => store.get_target(target).cloned(),
            Self::Literal(value) => Some(value.clone()),
            Self::Template(template) => Some(resolve_store_template(store, template)),
            Self::Object(entries) => Some(Value::Object(
                entries
                    .iter()
                    .map(|(key, binding)| (key.clone(), binding.resolve_nested(store)))
                    .collect(),
            )),
            Self::List(items) => Some(Value::List(
                items
                    .iter()
                    .map(|item| item.resolve_nested(store))
                    .collect(),
            )),
        }
    }

    fn resolve_nested(&self, store: &ValueStore) -> Value {
        match self {
            Self::Selector(target) => store.get_target(target).cloned().unwrap_or(Value::None),
            Self::Literal(value) => value.clone(),
            Self::Template(template) => resolve_store_template(store, template),
            Self::Object(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(key, binding)| (key.clone(), binding.resolve_nested(store)))
                    .collect(),
            ),
            Self::List(items) => Value::List(
                items
                    .iter()
                    .map(|item| item.resolve_nested(store))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WriteBinding {
    pub target: ValueTarget,
    pub expr: WriteExpr,
}

#[derive(Debug, Clone)]
pub enum WriteExpr {
    ScopeRef(String),
    Template(String),
    Literal(Value),
    Object(IndexMap<String, WriteExpr>),
    List(Vec<WriteExpr>),
}

impl WriteExpr {
    pub fn resolve_in_scope(&self, scope: &Value) -> Value {
        match self {
            Self::ScopeRef(path) => resolve_scope_ref(scope, path).unwrap_or(Value::None),
            Self::Template(template) => Value::Text(render_template(scope, template)),
            Self::Literal(value) => value.clone(),
            Self::Object(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(key, expr)| (key.clone(), expr.resolve(scope)))
                    .collect(),
            ),
            Self::List(items) => {
                Value::List(items.iter().map(|item| item.resolve(scope)).collect())
            }
        }
    }

    fn resolve(&self, scope: &Value) -> Value {
        self.resolve_in_scope(scope)
    }
}

pub fn bind_node(node: Node, binding: StoreBinding) -> Node {
    if binding.is_empty() {
        return node;
    }

    match node {
        Node::Input(widget) => Node::Input(Box::new(BoundInteractiveNode {
            inner: widget,
            binding,
            last_resolved_options: None,
            last_resolved_read: None,
        })),
        Node::Component(widget) => Node::Component(Box::new(BoundComponentNode {
            inner: widget,
            binding,
            last_resolved_options: None,
            last_resolved_read: None,
        })),
        Node::Output(widget) => Node::Output(Box::new(BoundOutputNode {
            inner: widget,
            binding,
        })),
    }
}

struct BoundInteractiveNode {
    inner: Box<dyn crate::widgets::traits::InteractiveNode>,
    binding: StoreBinding,
    last_resolved_options: Option<Option<Value>>,
    last_resolved_read: Option<Option<Value>>,
}

struct BoundComponentNode {
    inner: Box<dyn Component>,
    binding: StoreBinding,
    last_resolved_options: Option<Option<Value>>,
    last_resolved_read: Option<Option<Value>>,
}

impl BoundInteractiveNode {
    fn wrap_result(
        &self,
        before: Option<Value>,
        mut result: InteractionResult,
        after: Option<Value>,
    ) -> InteractionResult {
        if before != after {
            result.handled = true;
            result.request_render = true;
            result.actions.extend(
                self.binding
                    .write_changes(after)
                    .into_iter()
                    .map(|change| WidgetAction::ValueChanged { change }),
            );
        }
        result
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        let options_changed = sync_bound_options(
            store,
            &mut *self.inner,
            &self.binding,
            &mut self.last_resolved_options,
        );
        let value_changed = sync_bound_value(
            store,
            &mut *self.inner,
            &self.binding,
            &mut self.last_resolved_read,
            self.binding.interactive_read_mode(),
        );
        let inner_changed = self.inner.sync_from_store(store);
        options_changed || value_changed || inner_changed
    }
}

impl BoundComponentNode {
    fn wrap_result(
        &self,
        before: Option<Value>,
        mut result: InteractionResult,
        after: Option<Value>,
    ) -> InteractionResult {
        if before != after {
            result.handled = true;
            result.request_render = true;
            result.actions.extend(
                self.binding
                    .write_changes(after)
                    .into_iter()
                    .map(|change| WidgetAction::ValueChanged { change }),
            );
        }
        result
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        let options_changed = sync_bound_options(
            store,
            &mut *self.inner,
            &self.binding,
            &mut self.last_resolved_options,
        );
        let value_changed = sync_bound_value(
            store,
            &mut *self.inner,
            &self.binding,
            &mut self.last_resolved_read,
            self.binding.interactive_read_mode(),
        );
        let inner_changed = self.inner.sync_from_store(store);
        options_changed || value_changed || inner_changed
    }
}

impl Drawable for BoundInteractiveNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        self.inner.draw(ctx)
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        self.inner.pointer_rows(ctx)
    }

    fn hints(
        &self,
        ctx: crate::widgets::traits::HintContext,
    ) -> Vec<crate::widgets::traits::HintItem> {
        self.inner.hints(ctx)
    }
}

impl Interactive for BoundInteractiveNode {
    fn focus_mode(&self) -> FocusMode {
        self.inner.focus_mode()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        self.inner.overlay_placement()
    }

    fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.inner.overlay_open(saved_focus_id)
    }

    fn overlay_close(&mut self) -> Option<String> {
        self.inner.overlay_close()
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.inner.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_key(key);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_pointer(event);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        self.inner.text_editing()
    }

    fn on_text_edited(&mut self) {
        self.inner.on_text_edited();
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_text_action(action);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.inner.completion()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_system_event(event);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn on_tick(&mut self) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_tick();
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        self.inner.cursor_pos()
    }

    fn cursor_visible(&self) -> bool {
        self.inner.cursor_visible()
    }

    fn value(&self) -> Option<Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: Value) {
        self.inner.set_value(value);
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        BoundInteractiveNode::sync_from_store(self, store)
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        self.inner.validate(mode)
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }

    fn task_subscriptions(&self) -> Vec<TaskSubscription> {
        self.inner.task_subscriptions()
    }

    fn store_binding(&self) -> Option<&StoreBinding> {
        Some(&self.binding)
    }
}

impl Drawable for BoundComponentNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        self.inner.draw(ctx)
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        self.inner.pointer_rows(ctx)
    }

    fn hints(
        &self,
        ctx: crate::widgets::traits::HintContext,
    ) -> Vec<crate::widgets::traits::HintItem> {
        self.inner.hints(ctx)
    }
}

impl Interactive for BoundComponentNode {
    fn focus_mode(&self) -> FocusMode {
        self.inner.focus_mode()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        self.inner.overlay_placement()
    }

    fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.inner.overlay_open(saved_focus_id)
    }

    fn overlay_close(&mut self) -> Option<String> {
        self.inner.overlay_close()
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.inner.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_key(key);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_pointer(event);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        self.inner.text_editing()
    }

    fn on_text_edited(&mut self) {
        self.inner.on_text_edited();
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_text_action(action);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.inner.completion()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_system_event(event);
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn on_tick(&mut self) -> InteractionResult {
        let before = self.inner.value();
        let result = self.inner.on_tick();
        let after = self.inner.value();
        self.wrap_result(before, result, after)
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        self.inner.cursor_pos()
    }

    fn cursor_visible(&self) -> bool {
        self.inner.cursor_visible()
    }

    fn value(&self) -> Option<Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: Value) {
        self.inner.set_value(value);
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        BoundComponentNode::sync_from_store(self, store)
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        self.inner.validate(mode)
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }

    fn task_subscriptions(&self) -> Vec<TaskSubscription> {
        self.inner.task_subscriptions()
    }

    fn store_binding(&self) -> Option<&StoreBinding> {
        Some(&self.binding)
    }
}

impl Component for BoundComponentNode {
    fn children(&self) -> &[Node] {
        self.inner.children()
    }

    fn children_mut(&mut self) -> &mut [Node] {
        self.inner.children_mut()
    }
}

struct BoundOutputNode {
    inner: Box<dyn OutputNode>,
    binding: StoreBinding,
}

impl Drawable for BoundOutputNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        self.inner.draw(ctx)
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        self.inner.pointer_rows(ctx)
    }

    fn hints(
        &self,
        ctx: crate::widgets::traits::HintContext,
    ) -> Vec<crate::widgets::traits::HintItem> {
        self.inner.hints(ctx)
    }
}

impl OutputNode for BoundOutputNode {
    fn value(&self) -> Option<Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: Value) {
        self.inner.set_value(value);
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        let binding_changed = if let Some(value) = self.binding.read_value(store) {
            if self.inner.value().as_ref() == Some(&value) {
                false
            } else {
                self.inner.set_value(value);
                true
            }
        } else {
            false
        };
        let inner_changed = self.inner.sync_from_store(store);
        if binding_changed || inner_changed {
            return true;
        }
        false
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        self.inner.on_pointer(event)
    }

    fn on_tick(&mut self) -> InteractionResult {
        self.inner.on_tick()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        self.inner.on_system_event(event)
    }

    fn validate(&self) -> Result<(), String> {
        self.inner.validate()
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }

    fn task_subscriptions(&self) -> Vec<TaskSubscription> {
        self.inner.task_subscriptions()
    }

    fn store_binding(&self) -> Option<&StoreBinding> {
        Some(&self.binding)
    }
}

fn build_scope(reads: Option<&ReadBinding>, value: &Value) -> Value {
    let mut store = ValueStore::new();
    let _ = store.set("value", value.clone());
    insert_scope_paths(&mut store, "value", value);

    if let Some(ReadBinding::Selector(selector)) = reads {
        let _ = store.set_target(selector, value.clone());
    }

    if let Value::Object(entries) = value {
        for (key, nested) in entries {
            let _ = store.set(key.clone(), nested.clone());
            insert_scope_paths(&mut store, key.as_str(), nested);
        }
    }

    Value::Object(
        store
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
    )
}

fn sync_bound_value(
    store: &ValueStore,
    node: &mut dyn crate::widgets::traits::Interactive,
    binding: &StoreBinding,
    last_resolved_read: &mut Option<Option<Value>>,
    mode: InteractiveReadMode,
) -> bool {
    let Some(next) = binding.reads.as_ref().map(|reads| reads.resolve(store)) else {
        return false;
    };
    match mode {
        InteractiveReadMode::Controlled => {
            *last_resolved_read = Some(next.clone());
            let Some(value) = next else {
                return false;
            };
            if node.value().as_ref() == Some(&value) {
                return false;
            }
            node.set_value(value);
            true
        }
        InteractiveReadMode::Seeded => {
            let changed = last_resolved_read
                .as_ref()
                .is_none_or(|previous| previous != &next);
            if !changed {
                return false;
            }
            *last_resolved_read = Some(next.clone());
            let Some(value) = next else {
                return false;
            };
            if node.value().as_ref() == Some(&value) {
                return false;
            }
            node.set_value(value);
            true
        }
    }
}

fn sync_bound_options(
    store: &ValueStore,
    node: &mut dyn crate::widgets::traits::Interactive,
    binding: &StoreBinding,
    last_resolved_options: &mut Option<Option<Value>>,
) -> bool {
    let Some(next) = binding
        .options
        .as_ref()
        .map(|options| options.resolve(store))
    else {
        return false;
    };
    if last_resolved_options.as_ref() == Some(&next) {
        return false;
    }
    *last_resolved_options = Some(next.clone());
    let Some(value) = next else {
        return false;
    };
    node.set_options_from_value(value)
}

fn insert_scope_paths(store: &mut ValueStore, prefix: &str, value: &Value) {
    match value {
        Value::Object(entries) => {
            for (key, nested) in entries {
                let flat = format!("{prefix}.{key}");
                let _ = store.set(flat.clone(), nested.clone());
                insert_scope_paths(store, flat.as_str(), nested);
            }
        }
        Value::List(items) => {
            for (index, nested) in items.iter().enumerate() {
                let flat = format!("{prefix}[{index}]");
                let _ = store.set(flat.clone(), nested.clone());
                insert_scope_paths(store, flat.as_str(), nested);
            }
        }
        _ => {}
    }
}

fn resolve_scope_ref(scope: &Value, path: &str) -> Option<Value> {
    let Value::Object(map) = scope else {
        return None;
    };

    if let Some(value) = map.get(path) {
        return Some(value.clone());
    }

    let parsed = ValuePath::parse(path).ok()?;
    let (first, rest) = parsed.segments().split_first()?;
    let PathSegment::Key(root_key) = first else {
        return None;
    };

    let current = map.get(root_key.as_str())?;
    if rest.is_empty() {
        return Some(current.clone());
    }

    let nested = ValuePath::new(rest.to_vec());
    current.get_path(&nested).cloned()
}

fn resolve_store_ref(store: &ValueStore, path: &str) -> Option<Value> {
    let target = parse_store_selector(path).ok()?;
    store.get_target(&target).cloned()
}

fn resolve_store_template(store: &ValueStore, template: &str) -> Value {
    resolve_template_value(template, |expr| resolve_store_ref(store, expr))
}

fn render_template(scope: &Value, template: &str) -> String {
    render_resolved_template(
        template,
        |expr| resolve_scope_ref(scope, expr),
        |value| value.to_text_scalar().unwrap_or_else(|| value.to_json()),
    )
}
