use crate::core::component::{Component, FocusMode};
use crate::inputs::Input;

pub type NodeId = String;

pub enum Node {
    Input(Box<dyn Input>),
    Text(String),
    Component(Box<dyn Component>),
}

impl Node {
    pub fn input(input: impl Input + 'static) -> Self {
        Node::Input(Box::new(input))
    }

    pub fn text(content: impl Into<String>) -> Self {
        Node::Text(content.into())
    }

    pub fn component(component: impl Component + 'static) -> Self {
        Node::Component(Box::new(component))
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Node::Input(input) => Some(input.id()),
            Node::Component(component) => Some(component.id()),
            _ => None,
        }
    }

    pub fn as_input(&self) -> Option<&dyn Input> {
        match self {
            Node::Input(input) => Some(input.as_ref()),
            _ => None,
        }
    }

    pub fn as_input_mut(&mut self) -> Option<&mut dyn Input> {
        match self {
            Node::Input(input) => Some(input.as_mut()),
            _ => None,
        }
    }

    pub fn as_component(&self) -> Option<&dyn Component> {
        match self {
            Node::Component(component) => Some(component.as_ref()),
            _ => None,
        }
    }

    pub fn as_component_mut(&mut self) -> Option<&mut dyn Component> {
        match self {
            Node::Component(component) => Some(component.as_mut()),
            _ => None,
        }
    }

    pub fn is_input(&self) -> bool {
        matches!(self, Node::Input(_))
    }

    pub fn is_component(&self) -> bool {
        matches!(self, Node::Component(_))
    }

    pub fn focus_mode(&self) -> FocusMode {
        match self {
            Node::Component(component) => component.focus_mode(),
            _ => FocusMode::PassThrough,
        }
    }

    pub fn is_focusable(&self) -> bool {
        match self {
            Node::Input(_) => true,
            Node::Component(component) => matches!(component.focus_mode(), FocusMode::Group),
            _ => false,
        }
    }

    pub fn is_focused(&self) -> bool {
        match self {
            Node::Input(input) => input.is_focused(),
            Node::Component(component) => component.is_focused(),
            _ => false,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        match self {
            Node::Input(input) => input.set_focused(focused),
            Node::Component(component) => component.set_focused(focused),
            _ => {}
        }
    }

    pub fn children(&self) -> Option<&[Node]> {
        match self {
            Node::Component(component) => component.children(),
            _ => None,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut [Node]> {
        match self {
            Node::Component(component) => component.children_mut(),
            _ => None,
        }
    }
}

pub fn find_input<'a>(nodes: &'a [Node], id: &str) -> Option<&'a dyn Input> {
    for node in nodes {
        match node {
            Node::Input(input) => {
                if input.id() == id {
                    return Some(input.as_ref());
                }
            }
            Node::Component(component) => {
                if let Some(children) = component.children() {
                    if let Some(found) = find_input(children, id) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_input_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut dyn Input> {
    for node in nodes {
        match node {
            Node::Input(input) => {
                if input.id() == id {
                    return Some(input.as_mut());
                }
            }
            Node::Component(component) => {
                if let Some(children) = component.children_mut() {
                    if let Some(found) = find_input_mut(children, id) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_component<'a>(nodes: &'a [Node], id: &str) -> Option<&'a dyn Component> {
    for node in nodes {
        match node {
            Node::Component(component) => {
                if component.id() == id {
                    return Some(component.as_ref());
                }
                if let Some(children) = component.children() {
                    if let Some(found) = find_component(children, id) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_component_mut<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut dyn Component> {
    for node in nodes {
        match node {
            Node::Component(component) => {
                if component.id() == id {
                    return Some(component.as_mut());
                }
                if let Some(children) = component.children_mut() {
                    if let Some(found) = find_component_mut(children, id) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn first_input(nodes: &[Node]) -> Option<&dyn Input> {
    for node in nodes {
        match node {
            Node::Input(input) => return Some(input.as_ref()),
            Node::Component(component) => {
                if let Some(children) = component.children() {
                    if let Some(found) = first_input(children) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn first_input_mut(nodes: &mut [Node]) -> Option<&mut dyn Input> {
    for node in nodes {
        match node {
            Node::Input(input) => return Some(input.as_mut()),
            Node::Component(component) => {
                if let Some(children) = component.children_mut() {
                    if let Some(found) = first_input_mut(children) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn poll_components(nodes: &mut [Node]) -> bool {
    let mut updated = false;
    for node in nodes {
        match node {
            Node::Component(component) => {
                if component.poll() {
                    updated = true;
                }
                if let Some(children) = component.children_mut() {
                    if poll_components(children) {
                        updated = true;
                    }
                }
            }
            Node::Input(_) | Node::Text(_) => {}
        }
    }
    updated
}
