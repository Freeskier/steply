use crate::core::value::Value;
use crate::runtime::event::SystemEvent;
use crate::terminal::{CursorPos, KeyEvent};
use crate::widgets::traits::{
    CompletionState, DrawOutput, FocusMode, HintContext, HintItem, InteractionResult,
    InteractiveNode, OutputNode, OverlayMode, OverlayPlacement, RenderContext, TextAction,
    ValidationMode,
};











pub trait Component: InteractiveNode {
    fn children(&self) -> &[Node];
    fn children_mut(&mut self) -> &mut [Node];
}












#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeWalkScope {
    Visible,
    Persistent,
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

fn children_for_scope<'a>(node: &'a Node, scope: NodeWalkScope) -> Option<&'a [Node]> {
    let Node::Component(c) = node else {
        return None;
    };
    match scope {

        NodeWalkScope::Visible => None,

        NodeWalkScope::Persistent => Some(c.children()),
    }
}

fn children_for_scope_mut<'a>(node: &'a mut Node, scope: NodeWalkScope) -> Option<&'a mut [Node]> {
    let Node::Component(c) = node else {
        return None;
    };
    match scope {
        NodeWalkScope::Visible => None,
        NodeWalkScope::Persistent => Some(c.children_mut()),
    }
}





impl Node {
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

    pub fn focus_mode(&self) -> FocusMode {
        match self {
            Self::Input(w) => w.focus_mode(),
            Self::Component(w) => w.focus_mode(),
            Self::Output(_) => FocusMode::None,
        }
    }

    pub fn is_focusable(&self) -> bool {
        matches!(self.focus_mode(), FocusMode::Leaf | FocusMode::Group)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match self {
            Self::Input(w) => w.on_key(key),
            Self::Component(w) => w.on_key(key),
            Self::Output(_) => InteractionResult::ignored(),
        }
    }

    pub fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        match self {
            Self::Input(w) => w.on_text_action(action),
            Self::Component(w) => w.on_text_action(action),
            Self::Output(_) => InteractionResult::ignored(),
        }
    }

    pub fn on_text_edited(&mut self) {
        match self {
            Self::Input(w) => w.on_text_edited(),
            Self::Component(w) => w.on_text_edited(),
            Self::Output(_) => {}
        }
    }

    pub fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        match self {
            Self::Input(w) => w.on_system_event(event),
            Self::Component(w) => w.on_system_event(event),
            Self::Output(w) => w.on_system_event(event),
        }
    }

    pub fn completion(&mut self) -> Option<CompletionState<'_>> {
        match self {
            Self::Input(w) => w.completion(),
            Self::Component(w) => w.completion(),
            Self::Output(_) => None,
        }
    }

    pub fn on_tick(&mut self) -> InteractionResult {
        match self {
            Self::Input(w) => w.on_tick(),
            Self::Component(w) => w.on_tick(),
            Self::Output(w) => w.on_tick(),
        }
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        match self {
            Self::Input(w) => w.cursor_pos(),
            Self::Component(w) => w.cursor_pos(),
            Self::Output(_) => None,
        }
    }

    pub fn value(&self) -> Option<Value> {
        match self {
            Self::Input(w) => w.value(),
            Self::Component(w) => w.value(),
            Self::Output(w) => w.value(),
        }
    }

    pub fn set_value(&mut self, value: Value) {
        match self {
            Self::Input(w) => w.set_value(value),
            Self::Component(w) => w.set_value(value),
            Self::Output(w) => w.set_value(value),
        }
    }

    pub fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        match self {
            Self::Input(w) => w.validate(mode),
            Self::Component(w) => w.validate(mode),
            Self::Output(w) => w.validate(),
        }
    }



    pub fn overlay_placement(&self) -> Option<OverlayPlacement> {
        match self {
            Self::Input(w) => w.overlay_placement(),
            Self::Component(w) => w.overlay_placement(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        match self {
            Self::Input(w) => w.overlay_open(saved_focus_id),
            Self::Component(w) => w.overlay_open(saved_focus_id),
            Self::Output(_) => false,
        }
    }

    pub fn overlay_close(&mut self) -> Option<String> {
        match self {
            Self::Input(w) => w.overlay_close(),
            Self::Component(w) => w.overlay_close(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_mode(&self) -> OverlayMode {
        match self {
            Self::Input(w) => w.overlay_mode(),
            Self::Component(w) => w.overlay_mode(),
            Self::Output(_) => OverlayMode::Exclusive,
        }
    }



    pub fn persistent_children(&self) -> Option<&[Node]> {
        match self {
            Self::Component(c) => Some(c.children()),
            _ => None,
        }
    }

    pub fn persistent_children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Self::Component(c) => Some(c.children_mut()),
            _ => None,
        }
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
