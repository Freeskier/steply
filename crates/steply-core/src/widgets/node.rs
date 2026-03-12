use crate::core::value::Value;
use crate::runtime::event::SystemEvent;
use crate::runtime::event::ValueChange;
use crate::state::store::ValueStore;
use crate::task::TaskSpec;
use crate::terminal::{CursorPos, KeyEvent, PointerEvent};
use crate::widgets::traits::{
    CompletionState, DrawOutput, FocusMode, HintContext, HintItem, InteractionResult,
    InteractiveNode, OutputNode, OverlayMode, OverlayPlacement, PointerRowMap, RenderContext,
    TextAction, ValidationMode,
};

pub trait Component: InteractiveNode {
    fn children(&self) -> &[Node];
    fn children_mut(&mut self) -> &mut [Node];
}

pub trait LeafComponent: InteractiveNode {}

pub trait StaticChildrenComponent: LeafComponent {}

impl<T: LeafComponent + ?Sized> StaticChildrenComponent for T {}

impl<T> Component for T
where
    T: LeafComponent,
{
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeWalkScope {
    TopLevel,
    Recursive,
}

impl NodeWalkScope {
    #[allow(non_upper_case_globals)]
    pub const Visible: Self = Self::TopLevel;

    #[allow(non_upper_case_globals)]
    pub const Persistent: Self = Self::Recursive;
}

pub enum Node {
    Input(Box<dyn InteractiveNode>),

    Component(Box<dyn Component>),

    Output(Box<dyn OutputNode>),
}

pub fn walk_nodes(nodes: &[Node], scope: NodeWalkScope, f: &mut impl FnMut(&Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = children_for_scope(node, scope) {
            walk_nodes(children, scope, f);
        }
    }
}

pub fn walk_nodes_mut(nodes: &mut [Node], scope: NodeWalkScope, f: &mut impl FnMut(&mut Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = children_for_scope_mut(node, scope) {
            walk_nodes_mut(children, scope, f);
        }
    }
}

fn children_for_scope(node: &Node, scope: NodeWalkScope) -> Option<&[Node]> {
    let Node::Component(c) = node else {
        return None;
    };
    match scope {
        NodeWalkScope::TopLevel => None,

        NodeWalkScope::Recursive => Some(c.children()),
    }
}

fn children_for_scope_mut(node: &mut Node, scope: NodeWalkScope) -> Option<&mut [Node]> {
    let Node::Component(c) = node else {
        return None;
    };
    match scope {
        NodeWalkScope::TopLevel => None,
        NodeWalkScope::Recursive => Some(c.children_mut()),
    }
}

impl Node {
    fn interactive_ref(&self) -> Option<&dyn InteractiveNode> {
        match self {
            Self::Input(widget) => Some(widget.as_ref()),
            Self::Component(widget) => Some(widget.as_ref()),
            Self::Output(_) => None,
        }
    }

    fn interactive_mut(&mut self) -> Option<&mut dyn InteractiveNode> {
        match self {
            Self::Input(widget) => Some(widget.as_mut()),
            Self::Component(widget) => Some(widget.as_mut()),
            Self::Output(_) => None,
        }
    }

    fn output_ref(&self) -> Option<&dyn OutputNode> {
        match self {
            Self::Output(widget) => Some(widget.as_ref()),
            _ => None,
        }
    }

    fn output_mut(&mut self) -> Option<&mut dyn OutputNode> {
        match self {
            Self::Output(widget) => Some(widget.as_mut()),
            _ => None,
        }
    }

    fn component_ref(&self) -> Option<&dyn Component> {
        match self {
            Self::Component(component) => Some(component.as_ref()),
            _ => None,
        }
    }

    fn component_mut(&mut self) -> Option<&mut dyn Component> {
        match self {
            Self::Component(component) => Some(component.as_mut()),
            _ => None,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Input(w) => w.id(),
            Self::Component(w) => w.id(),
            Self::Output(w) => w.id(),
        }
    }

    pub fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        match self {
            Self::Input(w) => w.draw(ctx),
            Self::Component(w) => w.draw(ctx),
            Self::Output(w) => w.draw(ctx),
        }
    }

    pub fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        match self {
            Self::Input(w) => w.hints(ctx),
            Self::Component(w) => w.hints(ctx),
            Self::Output(w) => w.hints(ctx),
        }
    }

    pub fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        match self {
            Self::Input(w) => w.pointer_rows(ctx),
            Self::Component(w) => w.pointer_rows(ctx),
            Self::Output(w) => w.pointer_rows(ctx),
        }
    }

    pub fn focus_mode(&self) -> FocusMode {
        self.interactive_ref()
            .map(|widget| widget.focus_mode())
            .unwrap_or(FocusMode::None)
    }

    pub fn store_binding(&self) -> Option<&crate::widgets::shared::binding::StoreBinding> {
        if let Some(widget) = self.interactive_ref() {
            widget.store_binding()
        } else if let Some(widget) = self.output_ref() {
            widget.store_binding()
        } else {
            None
        }
    }

    pub fn is_focusable(&self) -> bool {
        matches!(self.focus_mode(), FocusMode::Leaf | FocusMode::Group)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        self.interactive_mut()
            .map(|widget| widget.on_key(key))
            .unwrap_or_else(InteractionResult::ignored)
    }

    pub fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        if let Some(widget) = self.interactive_mut() {
            widget.on_pointer(event)
        } else if let Some(widget) = self.output_mut() {
            widget.on_pointer(event)
        } else {
            InteractionResult::ignored()
        }
    }

    pub fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        self.interactive_mut()
            .map(|widget| widget.on_text_action(action))
            .unwrap_or_else(InteractionResult::ignored)
    }

    pub fn on_text_edited(&mut self) {
        if let Some(widget) = self.interactive_mut() {
            widget.on_text_edited();
        }
    }

    pub fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        if let Some(widget) = self.interactive_mut() {
            widget.on_system_event(event)
        } else if let Some(widget) = self.output_mut() {
            widget.on_system_event(event)
        } else {
            InteractionResult::ignored()
        }
    }

    pub fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.interactive_mut()
            .and_then(|widget| widget.completion())
    }

    pub fn on_tick(&mut self) -> InteractionResult {
        if let Some(widget) = self.interactive_mut() {
            widget.on_tick()
        } else if let Some(widget) = self.output_mut() {
            widget.on_tick()
        } else {
            InteractionResult::ignored()
        }
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        self.interactive_ref()
            .and_then(|widget| widget.cursor_pos())
    }

    pub fn cursor_visible(&self) -> bool {
        self.interactive_ref()
            .map(|widget| widget.cursor_visible())
            .unwrap_or(false)
    }

    pub fn value(&self) -> Option<Value> {
        if let Some(widget) = self.interactive_ref() {
            widget.value()
        } else if let Some(widget) = self.output_ref() {
            widget.value()
        } else {
            None
        }
    }

    pub fn set_value(&mut self, value: Value) {
        if let Some(widget) = self.interactive_mut() {
            widget.set_value(value);
        } else if let Some(widget) = self.output_mut() {
            widget.set_value(value);
        }
    }

    pub fn sync_from_store(&mut self, store: &ValueStore) -> bool {
        if let Some(widget) = self.interactive_mut() {
            widget.sync_from_store(store)
        } else if let Some(widget) = self.output_mut() {
            widget.sync_from_store(store)
        } else {
            false
        }
    }

    pub fn write_changes(&self) -> Vec<ValueChange> {
        if let Some(binding) = self.store_binding() {
            return binding.write_changes(self.value());
        }
        Vec::new()
    }

    pub fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        if let Some(widget) = self.interactive_ref() {
            widget.validate(mode)
        } else if let Some(widget) = self.output_ref() {
            widget.validate()
        } else {
            Ok(())
        }
    }

    pub fn overlay_placement(&self) -> Option<OverlayPlacement> {
        self.interactive_ref()
            .and_then(|widget| widget.overlay_placement())
    }

    pub fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.interactive_mut()
            .is_some_and(|widget| widget.overlay_open(saved_focus_id))
    }

    pub fn overlay_close(&mut self) -> Option<String> {
        self.interactive_mut()
            .and_then(|widget| widget.overlay_close())
    }

    pub fn overlay_mode(&self) -> OverlayMode {
        self.interactive_ref()
            .map(|widget| widget.overlay_mode())
            .unwrap_or(OverlayMode::Exclusive)
    }

    pub fn task_specs(&self) -> Vec<TaskSpec> {
        if let Some(widget) = self.interactive_ref() {
            widget.task_specs()
        } else if let Some(widget) = self.output_ref() {
            widget.task_specs()
        } else {
            Vec::new()
        }
    }

    pub fn persistent_children(&self) -> Option<&[Node]> {
        self.component_ref().map(Component::children)
    }

    pub fn persistent_children_mut(&mut self) -> Option<&mut [Node]> {
        self.component_mut().map(Component::children_mut)
    }
}

pub fn find_node<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.persistent_children()
            && let Some(found) = find_node(children, id)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_node_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.persistent_children_mut()
            && let Some(found) = find_node_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_overlay<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id() == id && node.overlay_placement().is_some() {
            return Some(node);
        }
        if let Some(children) = node.persistent_children()
            && let Some(found) = find_overlay(children, id)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_overlay_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for node in nodes {
        let is_target = node.id() == id && node.overlay_placement().is_some();
        if is_target {
            return Some(node);
        }
        if let Some(children) = node.persistent_children_mut()
            && let Some(found) = find_overlay_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}
