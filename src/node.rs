use crate::app::event::WidgetEvent;
use crate::domain::value::Value;
use crate::terminal::terminal::{CursorPos, KeyEvent};
use crate::widgets::traits::{
    DrawOutput, FocusMode, InteractionResult, InteractiveNode, RenderContext, RenderNode,
    TextAction,
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

pub fn visit_nodes_mut(nodes: &mut [Node], f: &mut impl FnMut(&mut Node)) {
    for node in nodes {
        f(node);
        if let Some(children) = node.children_mut() {
            visit_nodes_mut(children, f);
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

    pub fn set_focused(&mut self, focused: bool) {
        match self {
            Self::Input(w) | Self::Component(w) => w.set_focused(focused),
            Self::Output(_) => {}
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

pub fn apply_focus(nodes: &mut [Node], focused_id: Option<&str>) {
    for node in nodes {
        let focused = focused_id.is_some_and(|id| node.id() == id);
        node.set_focused(focused);
        if let Some(children) = node.children_mut() {
            apply_focus(children, focused_id);
        }
    }
}
