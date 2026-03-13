use crate::runtime::event::SystemEvent;
use crate::state::step::StepCondition;
use crate::state::store::ValueStore;
use crate::task::TaskSpec;
use crate::terminal::{CursorPos, KeyEvent, PointerEvent};
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintItem, InteractionResult,
    Interactive, InteractiveNode, OutputNode, OverlayMode, OverlayPlacement, PointerRowMap,
    RenderContext, StoreSyncPolicy, TextAction, ValidationMode,
};

pub fn wrap_node_when(node: Node, when: StepCondition) -> Node {
    match node {
        Node::Input(inner) => Node::Input(Box::new(ConditionalInputNode::new(inner, when))),
        Node::Component(inner) => {
            Node::Component(Box::new(ConditionalComponentNode::new(inner, when)))
        }
        Node::Output(inner) => Node::Output(Box::new(ConditionalOutputNode::new(inner, when))),
    }
}

struct ConditionalInputNode {
    inner: Box<dyn InteractiveNode>,
    when: StepCondition,
    visible: bool,
}

impl ConditionalInputNode {
    fn new(inner: Box<dyn InteractiveNode>, when: StepCondition) -> Self {
        Self {
            inner,
            when,
            visible: true,
        }
    }

    fn refresh_visibility(&mut self, store: &ValueStore) -> bool {
        let next = self.when.evaluate(store);
        let changed = self.visible != next;
        self.visible = next;
        changed
    }
}

impl Drawable for ConditionalInputNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        if self.visible {
            self.inner.draw(ctx)
        } else {
            DrawOutput::default()
        }
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        if self.visible {
            self.inner.pointer_rows(ctx)
        } else {
            Vec::new()
        }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if self.visible {
            self.inner.hints(ctx)
        } else {
            Vec::new()
        }
    }
}

impl Interactive for ConditionalInputNode {
    fn focus_mode(&self) -> FocusMode {
        if self.visible {
            self.inner.focus_mode()
        } else {
            FocusMode::None
        }
    }

    fn store_binding(&self) -> Option<&crate::widgets::shared::binding::StoreBinding> {
        self.inner.store_binding()
    }

    fn store_sync_policy(&self) -> StoreSyncPolicy {
        self.inner.store_sync_policy()
    }

    fn commit_policy(&self) -> crate::state::change::StoreCommitPolicy {
        self.inner.commit_policy()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        if self.visible {
            self.inner.overlay_placement()
        } else {
            None
        }
    }

    fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.visible && self.inner.overlay_open(saved_focus_id)
    }

    fn overlay_close(&mut self) -> Option<String> {
        if self.visible {
            self.inner.overlay_close()
        } else {
            None
        }
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.inner.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_key(key)
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_pointer(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn text_editing(&mut self) -> Option<crate::widgets::traits::TextEditState<'_>> {
        if self.visible {
            self.inner.text_editing()
        } else {
            None
        }
    }

    fn on_text_edited(&mut self) {
        if self.visible {
            self.inner.on_text_edited();
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.visible {
            self.inner.on_text_action(action)
        } else {
            InteractionResult::ignored()
        }
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if self.visible {
            self.inner.completion()
        } else {
            None
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_system_event(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_tick(&mut self) -> InteractionResult {
        if self.visible {
            self.inner.on_tick()
        } else {
            InteractionResult::ignored()
        }
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        self.visible.then(|| self.inner.cursor_pos()).flatten()
    }

    fn cursor_visible(&self) -> bool {
        self.visible && self.inner.cursor_visible()
    }

    fn value(&self) -> Option<crate::core::value::Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: crate::core::value::Value) {
        self.inner.set_value(value);
    }

    fn set_options_from_value(&mut self, value: crate::core::value::Value) -> bool {
        self.inner.set_options_from_value(value)
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        self.sync_from_store_with_focus(store, false)
    }

    fn sync_from_store_with_focus(&mut self, store: &ValueStore, is_focused: bool) -> bool {
        let visibility_changed = self.refresh_visibility(store);
        let inner_changed = self
            .inner
            .sync_from_store_with_focus(store, is_focused && self.visible);
        visibility_changed || inner_changed
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        if self.visible {
            self.inner.validate(mode)
        } else {
            Ok(())
        }
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }
}

struct ConditionalComponentNode {
    inner: Box<dyn Component>,
    when: StepCondition,
    visible: bool,
    hidden_children: Vec<Node>,
}

impl ConditionalComponentNode {
    fn new(inner: Box<dyn Component>, when: StepCondition) -> Self {
        Self {
            inner,
            when,
            visible: true,
            hidden_children: Vec::new(),
        }
    }

    fn refresh_visibility(&mut self, store: &ValueStore) -> bool {
        let next = self.when.evaluate(store);
        let changed = self.visible != next;
        self.visible = next;
        changed
    }
}

impl Drawable for ConditionalComponentNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        if self.visible {
            self.inner.draw(ctx)
        } else {
            DrawOutput::default()
        }
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        if self.visible {
            self.inner.pointer_rows(ctx)
        } else {
            Vec::new()
        }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if self.visible {
            self.inner.hints(ctx)
        } else {
            Vec::new()
        }
    }
}

impl Interactive for ConditionalComponentNode {
    fn focus_mode(&self) -> FocusMode {
        if self.visible {
            self.inner.focus_mode()
        } else {
            FocusMode::None
        }
    }

    fn store_binding(&self) -> Option<&crate::widgets::shared::binding::StoreBinding> {
        self.inner.store_binding()
    }

    fn store_sync_policy(&self) -> StoreSyncPolicy {
        self.inner.store_sync_policy()
    }

    fn commit_policy(&self) -> crate::state::change::StoreCommitPolicy {
        self.inner.commit_policy()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        if self.visible {
            self.inner.overlay_placement()
        } else {
            None
        }
    }

    fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.visible && self.inner.overlay_open(saved_focus_id)
    }

    fn overlay_close(&mut self) -> Option<String> {
        if self.visible {
            self.inner.overlay_close()
        } else {
            None
        }
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.inner.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_key(key)
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_pointer(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn text_editing(&mut self) -> Option<crate::widgets::traits::TextEditState<'_>> {
        if self.visible {
            self.inner.text_editing()
        } else {
            None
        }
    }

    fn on_text_edited(&mut self) {
        if self.visible {
            self.inner.on_text_edited();
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.visible {
            self.inner.on_text_action(action)
        } else {
            InteractionResult::ignored()
        }
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if self.visible {
            self.inner.completion()
        } else {
            None
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_system_event(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_tick(&mut self) -> InteractionResult {
        if self.visible {
            self.inner.on_tick()
        } else {
            InteractionResult::ignored()
        }
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        self.visible.then(|| self.inner.cursor_pos()).flatten()
    }

    fn cursor_visible(&self) -> bool {
        self.visible && self.inner.cursor_visible()
    }

    fn value(&self) -> Option<crate::core::value::Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: crate::core::value::Value) {
        self.inner.set_value(value);
    }

    fn set_options_from_value(&mut self, value: crate::core::value::Value) -> bool {
        self.inner.set_options_from_value(value)
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        self.sync_from_store_with_focus(store, false)
    }

    fn sync_from_store_with_focus(&mut self, store: &ValueStore, is_focused: bool) -> bool {
        let visibility_changed = self.refresh_visibility(store);
        let inner_changed = self
            .inner
            .sync_from_store_with_focus(store, is_focused && self.visible);
        visibility_changed || inner_changed
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        if self.visible {
            self.inner.validate(mode)
        } else {
            Ok(())
        }
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }
}

impl Component for ConditionalComponentNode {
    fn children(&self) -> &[Node] {
        if self.visible {
            self.inner.children()
        } else {
            &[]
        }
    }

    fn children_mut(&mut self) -> &mut [Node] {
        if self.visible {
            self.inner.children_mut()
        } else {
            self.hidden_children.as_mut_slice()
        }
    }
}

struct ConditionalOutputNode {
    inner: Box<dyn OutputNode>,
    when: StepCondition,
    visible: bool,
}

impl ConditionalOutputNode {
    fn new(inner: Box<dyn OutputNode>, when: StepCondition) -> Self {
        Self {
            inner,
            when,
            visible: true,
        }
    }

    fn refresh_visibility(&mut self, store: &ValueStore) -> bool {
        let next = self.when.evaluate(store);
        let changed = self.visible != next;
        self.visible = next;
        changed
    }
}

impl Drawable for ConditionalOutputNode {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        if self.visible {
            self.inner.draw(ctx)
        } else {
            DrawOutput::default()
        }
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        if self.visible {
            self.inner.pointer_rows(ctx)
        } else {
            Vec::new()
        }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if self.visible {
            self.inner.hints(ctx)
        } else {
            Vec::new()
        }
    }
}

impl OutputNode for ConditionalOutputNode {
    fn store_binding(&self) -> Option<&crate::widgets::shared::binding::StoreBinding> {
        self.inner.store_binding()
    }

    fn value(&self) -> Option<crate::core::value::Value> {
        self.inner.value()
    }

    fn set_value(&mut self, value: crate::core::value::Value) {
        self.inner.set_value(value);
    }

    fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        self.sync_from_store_with_focus(store, false)
    }

    fn sync_from_store_with_focus(&mut self, store: &ValueStore, is_focused: bool) -> bool {
        let visibility_changed = self.refresh_visibility(store);
        let inner_changed = self
            .inner
            .sync_from_store_with_focus(store, is_focused && self.visible);
        visibility_changed || inner_changed
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_pointer(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_tick(&mut self) -> InteractionResult {
        if self.visible {
            self.inner.on_tick()
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        if self.visible {
            self.inner.on_system_event(event)
        } else {
            InteractionResult::ignored()
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.visible {
            self.inner.validate()
        } else {
            Ok(())
        }
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.inner.task_specs()
    }
}
