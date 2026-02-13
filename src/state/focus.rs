use crate::core::NodeId;
use crate::widgets::node::Node;

#[derive(Debug, Clone)]
pub struct FocusTarget {
    pub id: NodeId,
}

#[derive(Debug, Default, Clone)]
pub struct FocusState {
    targets: Vec<FocusTarget>,
    index: Option<usize>,
}

impl FocusState {
    pub fn from_nodes(nodes: &[Node]) -> Self {
        let mut state = Self::default();
        state.rebuild(nodes);
        state
    }

    pub fn rebuild(&mut self, nodes: &[Node]) {
        self.targets.clear();
        collect_targets(nodes, &mut self.targets);
        self.index = if self.targets.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    pub fn current_id(&self) -> Option<&str> {
        self.index
            .and_then(|i| self.targets.get(i))
            .map(|target| target.id.as_str())
    }

    pub fn set_focus_by_id(&mut self, id: &str) {
        self.index = self
            .targets
            .iter()
            .position(|target| target.id.as_str() == id);
    }

    pub fn next(&mut self) {
        let Some(current) = self.index else {
            return;
        };
        if self.targets.is_empty() {
            self.index = None;
            return;
        }
        self.index = Some((current + 1) % self.targets.len());
    }

    pub fn prev(&mut self) {
        let Some(current) = self.index else {
            return;
        };
        if self.targets.is_empty() {
            self.index = None;
            return;
        }
        self.index = Some((current + self.targets.len() - 1) % self.targets.len());
    }
}

fn collect_targets(nodes: &[Node], out: &mut Vec<FocusTarget>) {
    for node in nodes {
        if node.is_focusable_leaf_or_group() {
            out.push(FocusTarget {
                id: node.id().into(),
            });
            continue;
        }

        if let Some(children) = node.visible_children() {
            collect_targets(children, out);
        }
    }
}
