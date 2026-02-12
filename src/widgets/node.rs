use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyEvent};
use crate::widgets::traits::{
    CompletionState, DrawOutput, FocusMode, InteractionResult, InteractiveNode, OverlayMode,
    OverlayPlacement, RenderContext, RenderNode, TextAction,
};

pub enum Node {
    Input(Box<dyn InteractiveNode>),
    Component(Box<dyn InteractiveNode>),
    Output(Box<dyn RenderNode>),
}

pub fn visit_nodes(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = node.children() {
            visit_nodes(children, f);
        }
    }
}

pub fn visit_state_nodes(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = node.state_children() {
            visit_state_nodes(children, f);
        }
    }
}

pub fn visit_nodes_mut(nodes: &mut [Node], f: &mut impl FnMut(&mut Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = node.children_mut() {
            visit_nodes_mut(children, f);
        }
    }
}

pub fn visit_state_nodes_mut(nodes: &mut [Node], f: &mut impl FnMut(&mut Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = node.state_children_mut() {
            visit_state_nodes_mut(children, f);
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

    pub fn completion_state(&mut self) -> Option<CompletionState<'_>> {
        match self {
            Self::Input(w) | Self::Component(w) => w.completion_state(),
            Self::Output(_) => None,
        }
    }

    pub fn on_tick(&mut self) -> InteractionResult {
        match self {
            Self::Input(w) | Self::Component(w) => w.on_tick(),
            Self::Output(_) => InteractionResult::ignored(),
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
            Self::Output(_) => {}
        }
    }

    pub fn value(&self) -> Option<Value> {
        match self {
            Self::Input(w) | Self::Component(w) => w.value(),
            Self::Output(_) => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Input(w) | Self::Component(w) => w.validate(),
            Self::Output(_) => Ok(()),
        }
    }

    pub fn children(&self) -> Option<&[Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.children(),
            Self::Output(_) => None,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.children_mut(),
            Self::Output(_) => None,
        }
    }

    pub fn state_children(&self) -> Option<&[Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.state_children(),
            Self::Output(_) => None,
        }
    }

    pub fn state_children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Self::Input(w) | Self::Component(w) => w.state_children_mut(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_placement(&self) -> Option<OverlayPlacement> {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_placement(),
            Self::Output(_) => None,
        }
    }

    pub fn overlay_is_visible(&self) -> bool {
        match self {
            Self::Input(w) | Self::Component(w) => w.overlay_is_visible(),
            Self::Output(_) => false,
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

pub fn find_node_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.children_mut()
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
        if let Some(children) = node.children()
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
        if let Some(children) = node.state_children()
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
        if let Some(children) = node.state_children_mut()
            && let Some(found) = find_overlay_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_visible_overlay(nodes: &[Node]) -> Option<&Node> {
    for node in nodes {
        if node.overlay_is_visible() {
            return Some(node);
        }
        if let Some(children) = node.state_children()
            && let Some(found) = find_visible_overlay(children)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_visible_overlay_mut(nodes: &mut [Node]) -> Option<&mut Node> {
    for node in nodes {
        if node.overlay_is_visible() {
            return Some(node);
        }
        if let Some(children) = node.state_children_mut()
            && let Some(found) = find_visible_overlay_mut(children)
        {
            return Some(found);
        }
    }
    None
}
