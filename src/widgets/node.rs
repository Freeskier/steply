use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyEvent};
use crate::widgets::traits::{
    CompletionState, DrawOutput, FocusMode, InteractionResult, InteractiveNode, OverlayMode,
    OverlayPlacement, RenderContext, RenderNode, TextAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeWalkScope {
    Visible,
    Persistent,
}

pub enum Node {
    Input(Box<dyn InteractiveNode>),
    Component(Box<dyn InteractiveNode>),
    Output(Box<dyn RenderNode>),
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

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Self::Input(w) | Self::Component(w) => w.id(),
            Self::Output(w) => w.id(),
        }
    }

    pub fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        match self {
            Self::Input(w) | Self::Component(w) => w.draw(ctx),
            Self::Output(w) => w.draw(ctx),
        }
    }

    pub fn is_focusable_leaf_or_group(&self) -> bool {
        match self {
            Self::Input(w) | Self::Component(w) => {
                matches!(w.focus_mode(), FocusMode::Leaf | FocusMode::Group)
            }
            Self::Output(_) => false,
        }
    }

    pub fn focus_mode(&self) -> FocusMode {
        match self {
            Self::Input(w) | Self::Component(w) => w.focus_mode(),
            Self::Output(_) => FocusMode::None,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match self {
            Self::Input(w) | Self::Component(w) => w.on_key(key),
            Self::Output(_) => InteractionResult::ignored(),
        }
    }

    pub fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        match self {
            Self::Input(w) | Self::Component(w) => w.on_text_action(action),
            Self::Output(_) => InteractionResult::ignored(),
        }
    }

    pub fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match self {
            Self::Input(w) | Self::Component(w) => w.on_event(event),
            Self::Output(_) => InteractionResult::ignored(),
        }
    }

    pub fn completion(&mut self) -> Option<CompletionState<'_>> {
        match self {
            Self::Input(w) | Self::Component(w) => w.completion(),
            Self::Output(_) => None,
        }
    }

    pub fn on_tick(&mut self) -> InteractionResult {
        match self {
            Self::Input(w) | Self::Component(w) => w.on_tick(),
            Self::Output(w) => {
                if w.on_tick() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
        }
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        match self {
            Self::Input(w) | Self::Component(w) => w.cursor_pos(),
            Self::Output(_) => None,
        }
    }

    pub fn set_value(&mut self, value: Value) {
        match self {
            Self::Input(w) | Self::Component(w) => w.set_value(value),
            Self::Output(w) => w.set_value(value),
        }
    }

    pub fn value(&self) -> Option<Value> {
        match self {
            Self::Input(w) | Self::Component(w) => w.value(),
            Self::Output(w) => w.value(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_submit()
    }

    pub fn validate_live(&self) -> Result<(), String> {
        match self {
            Self::Input(w) | Self::Component(w) => w.validate_live(),
            Self::Output(_) => Ok(()),
        }
    }

    pub fn validate_submit(&self) -> Result<(), String> {
        match self {
            Self::Input(w) | Self::Component(w) => w.validate_submit(),
            Self::Output(w) => w.validate(),
        }
    }

    pub fn visible_children(&self) -> Option<&[Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.visible_children(),
            Self::Output(_) => None,
        }
    }

    pub fn visible_children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.visible_children_mut(),
            Self::Output(_) => None,
        }
    }

    pub fn persistent_children(&self) -> Option<&[Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.persistent_children(),
            Self::Output(_) => None,
        }
    }

    pub fn persistent_children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.persistent_children_mut(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_placement(&self) -> Option<OverlayPlacement> {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_placement(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_open(saved_focus_id),
            Self::Output(_) => false,
        }
    }

    pub fn overlay_close(&mut self) -> Option<String> {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_close(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_mode(&self) -> OverlayMode {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_mode(),
            Self::Output(_) => OverlayMode::Exclusive,
        }
    }
}

fn children_for_scope<'a>(node: &'a Node, scope: NodeWalkScope) -> Option<&'a [Node]> {
    match scope {
        NodeWalkScope::Visible => node.visible_children(),
        NodeWalkScope::Persistent => node.persistent_children(),
    }
}

fn children_for_scope_mut<'a>(node: &'a mut Node, scope: NodeWalkScope) -> Option<&'a mut [Node]> {
    match scope {
        NodeWalkScope::Visible => node.visible_children_mut(),
        NodeWalkScope::Persistent => node.persistent_children_mut(),
    }
}

pub fn find_node_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.visible_children_mut()
            && let Some(found) = find_node_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_node<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.visible_children()
            && let Some(found) = find_node(children, id)
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
        let is_target_overlay = node.id() == id && node.overlay_placement().is_some();
        if is_target_overlay {
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
