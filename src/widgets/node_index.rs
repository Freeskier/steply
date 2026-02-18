use crate::core::NodeId;
use crate::widgets::node::{Node, NodeWalkScope};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodePath(Vec<usize>);

impl NodePath {
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

        // For Visible scope: don't descend into components (they draw themselves).
        // For Persistent scope: descend into component children.
        let children = match scope {
            NodeWalkScope::Visible => None,
            NodeWalkScope::Persistent => node.persistent_children(),
        };
        if let Some(children) = children {
            collect_paths(children, scope, path, out);
        }

        path.pop();
    }
}
