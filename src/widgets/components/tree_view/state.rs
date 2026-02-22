use crate::core::search::fuzzy::match_text;

use super::TreeItemLabel;
use super::TreeNode;

pub(super) fn rebuild_visible<T: TreeItemLabel>(nodes: &[TreeNode<T>]) -> Vec<usize> {
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

pub(super) fn rebuild_visible_filtered<T: TreeItemLabel>(
    nodes: &[TreeNode<T>],
    query: &str,
) -> Vec<usize> {
    let q = query.trim();
    if q.is_empty() {
        return rebuild_visible(nodes);
    }

    let mut parents = Vec::<Option<usize>>::with_capacity(nodes.len());
    let mut stack = Vec::<usize>::new();
    for (idx, node) in nodes.iter().enumerate() {
        stack.truncate(node.depth);
        parents.push(stack.last().copied());
        stack.push(idx);
    }

    let matched = nodes
        .iter()
        .map(|node| {
            let search = node.item.search_text();
            match_text(q, search.as_ref()).is_some()
        })
        .collect::<Vec<_>>();

    let mut has_match_subtree = matched.clone();
    for idx in (0..nodes.len()).rev() {
        if has_match_subtree[idx] {
            if let Some(parent) = parents[idx] {
                has_match_subtree[parent] = true;
            }
        }
    }

    struct AncestorState {
        open: bool,
        force_descendants: bool,
    }

    let mut visible = Vec::<usize>::new();
    let mut path = Vec::<AncestorState>::new();

    for idx in 0..nodes.len() {
        let depth = nodes[idx].depth;
        path.truncate(depth);

        let parent_open = path.last().map(|p| p.open).unwrap_or(true);
        let inherited_force = path.last().map(|p| p.force_descendants).unwrap_or(false);

        let show = parent_open && (has_match_subtree[idx] || inherited_force);
        if show {
            visible.push(idx);
        }

        let open = show && nodes[idx].has_children && nodes[idx].expanded;
        let force_descendants = open && (inherited_force || matched[idx]);
        path.push(AncestorState {
            open,
            force_descendants,
        });
    }

    visible
}
