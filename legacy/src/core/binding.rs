use crate::core::node::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindTarget {
    Input(NodeId),
    Component(NodeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueSource {
    Component(NodeId),
    Layer(String),
}
