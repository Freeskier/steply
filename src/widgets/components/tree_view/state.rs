use super::TreeNode;

pub(super) fn rebuild_visible<T: Send + 'static>(nodes: &[TreeNode<T>]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut expand_stack: Vec<bool> = Vec::new();
    let mut collapsed_ancestors = 0usize;

    for (idx, node) in nodes.iter().enumerate() {
        let d = node.depth;
        while expand_stack.len() > d {
            if let Some(expanded) = expand_stack.pop()
                && !expanded
            {
                collapsed_ancestors = collapsed_ancestors.saturating_sub(1);
            }
        }

        // Visible if root-level, or all ancestors on the current path are expanded.
        if d == 0 || collapsed_ancestors == 0 {
            visible.push(idx);
        }

        let expanded = node.has_children && node.expanded;
        expand_stack.push(expanded);
        if !expanded {
            collapsed_ancestors += 1;
        }
    }

    visible
}
