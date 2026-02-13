use crate::core::NodeId;
use crate::widgets::node::{Node, NodeWalkScope};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodePath(Vec<usize>);

impl NodePath {
    pub fn as_slice(&self) -> &[usize] {
        self.0.as_slice()
    }

    fn push(&mut self, index: usize) {
        self.0.push(index);
    }

    fn pop(&mut self) {
        let _ = self.0.pop();
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeIndex {
    visible: HashMap<NodeId, NodePath>,
    persistent: HashMap<NodeId, NodePath>,
}

impl NodeIndex {
    pub fn build(nodes: &[Node]) -> Self {
        let mut index = Self::default();
        let mut path = NodePath::default();
        collect_paths(nodes, NodeWalkScope::Visible, &mut path, &mut index.visible);
        path = NodePath::default();
        collect_paths(
            nodes,
            NodeWalkScope::Persistent,
            &mut path,
            &mut index.persistent,
        );
        index
    }

    pub fn has_visible(&self, id: &str) -> bool {
        self.visible.contains_key(id)
    }

    pub fn visible_path(&self, id: &str) -> Option<&NodePath> {
        self.visible.get(id)
    }
}

pub fn node_at_path_mut<'a>(
    roots: &'a mut [Node],
    path: &NodePath,
    scope: NodeWalkScope,
) -> Option<&'a mut Node> {
    node_at_path_slice_mut(roots, path.as_slice(), scope)
}

fn node_at_path_slice_mut<'a>(
    roots: &'a mut [Node],
    path: &[usize],
    scope: NodeWalkScope,
) -> Option<&'a mut Node> {
    let (&first, rest) = path.split_first()?;
    let node = roots.get_mut(first)?;
    if rest.is_empty() {
        return Some(node);
    }

    let children = match scope {
        NodeWalkScope::Visible => node.visible_children_mut()?,
        NodeWalkScope::Persistent => node.persistent_children_mut()?,
    };
    node_at_path_slice_mut(children, rest, scope)
}

fn collect_paths(
    nodes: &[Node],
    scope: NodeWalkScope,
    path: &mut NodePath,
    out: &mut HashMap<NodeId, NodePath>,
) {
    for (index, node) in nodes.iter().enumerate() {
        path.push(index);
        out.insert(node.id().into(), path.clone());

        let children = match scope {
            NodeWalkScope::Visible => node.visible_children(),
            NodeWalkScope::Persistent => node.persistent_children(),
        };
        if let Some(children) = children {
            collect_paths(children, scope, path, out);
        }

        path.pop();
    }
}
