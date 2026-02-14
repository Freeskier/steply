use super::TreeNode;

pub(super) fn rebuild_visible<T: Send + 'static>(nodes: &[TreeNode<T>]) -> Vec<usize> {
    let mut visible = Vec::new();
    // Stack of (depth, expanded) for ancestor tracking
    // We include a node if every ancestor at each depth level is expanded.
    // We track this with a depth-indexed stack: expanded_at_depth[d] = true
    // means the parent at depth d is currently expanded.
    //
    // Algorithm:
    // - Maintain a stack of ancestor depths that are "open" (expanded).
    // - A node at depth d is visible if all its ancestor nodes were expanded.
    // - We track `open_depth` = max depth we are currently "inside" (i.e., the
    //   deepest ancestor that is expanded). A node is visible if depth <= open_depth+1
    //   (meaning its parent was expanded or it is at depth 0).

    let mut expand_stack: Vec<bool> = Vec::new(); // per-depth: is this depth's last seen node expanded?

    for (idx, node) in nodes.iter().enumerate() {
        let d = node.depth;

        // Trim stack to current depth
        expand_stack.truncate(d);

        // A node is visible if:
        // - depth == 0 (root level), OR
        // - its ancestors are all expanded (stack has entries for all ancestor depths), OR
        // - its ancestors don't exist in nodes[] at all (orphan from query results —
        //   stack is shorter than d, meaning no parent was seen at depth d-1)
        let parent_expanded = d == 0
            || expand_stack.len() < d  // orphan: no parent node exists → show it
            || expand_stack.iter().all(|&e| e);

        if parent_expanded {
            visible.push(idx);
        }

        // Push this node's expanded state for its children (at depth d+1)
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

pub(super) fn clamp_active(active_index: &mut usize, visible_len: usize) {
    if visible_len == 0 {
        *active_index = 0;
    } else if *active_index >= visible_len {
        *active_index = visible_len - 1;
    }
}

pub(super) fn ensure_visible(
    scroll_offset: &mut usize,
    max_visible: Option<usize>,
    active_index: usize,
    visible_len: usize,
) {
    let Some(max_visible) = max_visible else {
        return;
    };

    if visible_len <= max_visible {
        *scroll_offset = 0;
        return;
    }

    if active_index < *scroll_offset {
        *scroll_offset = active_index;
        return;
    }

    let last_visible = scroll_offset.saturating_add(max_visible).saturating_sub(1);
    if active_index > last_visible {
        *scroll_offset = active_index + 1 - max_visible;
    }
}

pub(super) fn visible_range(
    scroll_offset: usize,
    max_visible: Option<usize>,
    visible_len: usize,
) -> (usize, usize) {
    match max_visible {
        Some(limit) => {
            let start = scroll_offset.min(visible_len);
            let end = (start + limit).min(visible_len);
            (start, end)
        }
        None => (0, visible_len),
    }
}
