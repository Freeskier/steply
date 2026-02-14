use super::TreeNode;

pub(super) fn rebuild_visible<T: Send + 'static>(nodes: &[TreeNode<T>]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut expand_stack: Vec<bool> = Vec::new();

    for (idx, node) in nodes.iter().enumerate() {
        let d = node.depth;
        expand_stack.truncate(d);

        // Visible if: root level, OR no ancestor exists in nodes[] (orphan from query),
        // OR all ancestors are expanded.
        let parent_expanded = d == 0 || expand_stack.len() < d || expand_stack.iter().all(|&e| e);

        if parent_expanded {
            visible.push(idx);
        }

        if expand_stack.len() == d {
            expand_stack.push(if node.has_children {
                node.expanded
            } else {
                false
            });
        }
    }

    visible
}
