use crate::core::component::Component;
use crate::inputs::Input;

pub type NodeId = String;

pub enum Node {
    Input(Box<dyn Input>),
    Text(String),
    Separator,
    Component(Box<dyn Component>),
}

impl Node {
    pub fn input(input: impl Input + 'static) -> Self {
        Node::Input(Box::new(input))
    }

    pub fn text(content: impl Into<String>) -> Self {
        Node::Text(content.into())
    }

    pub fn separator() -> Self {
        Node::Separator
    }

    pub fn component(component: impl Component + 'static) -> Self {
        Node::Component(Box::new(component))
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Node::Input(input) => Some(input.id()),
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
}
