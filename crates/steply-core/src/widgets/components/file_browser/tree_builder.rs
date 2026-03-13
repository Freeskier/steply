use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::widgets::components::tree_view::TreeNode;

pub(super) fn build_tree_nodes_for(
    browse_dir: &Path,
    show_parent_option: bool,
    result: &ScanResult,
    selected_paths: &[PathBuf],
    expanded_paths: &HashSet<PathBuf>,
    cached_subtrees: &HashMap<PathBuf, Vec<TreeNode<FileTreeItem>>>,
) -> Vec<TreeNode<FileTreeItem>> {
    let mut nodes = Vec::<TreeNode<FileTreeItem>>::new();

    if show_parent_option && let Some(parent) = browse_dir.parent() {
        let dotdot_entry = model::FileEntry {
            name: "..".to_string(),
            name_lower: "..".to_string(),
            ext_lower: None,
            path: Arc::new(parent.to_path_buf()),
            kind: model::EntryKind::Dir,
        };
        nodes.push(TreeNode::new(
            FileTreeItem::new(dotdot_entry, Vec::new(), false),
            0,
            false,
        ));
    }

    let mut ordered_entries = result
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| (idx, entry, tree_entry_key(entry, browse_dir)))
        .collect::<Vec<_>>();
    ordered_entries.sort_by(|(_, left, left_key), (_, right, right_key)| {
        left_key
            .cmp(right_key)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut inserted_dirs = HashSet::<PathBuf>::new();
    let mut node_index_by_path = HashMap::<PathBuf, usize>::new();
    for (entry_idx, entry, _) in ordered_entries {
        let highlights = result
            .highlights
            .get(entry_idx)
            .cloned()
            .unwrap_or_default();
        let rel = match entry.path.strip_prefix(browse_dir) {
            Ok(path) => path.to_path_buf(),
            Err(_) => {
                let idx = nodes.len();
                node_index_by_path.insert((*entry.path).clone(), idx);
                nodes.push(TreeNode::new(
                    FileTreeItem::new(
                        entry.clone(),
                        highlights,
                        selected_paths
                            .iter()
                            .any(|path| path == entry.path.as_ref()),
                    ),
                    0,
                    entry.kind.is_dir(),
                ));
                continue;
            }
        };

        let components: Vec<_> = rel.components().collect();
        let mut anc_abs = browse_dir.to_path_buf();
        for (anc_depth, comp) in components
            .iter()
            .take(components.len().saturating_sub(1))
            .enumerate()
        {
            anc_abs.push(comp.as_os_str());
            if inserted_dirs.insert(anc_abs.clone()) {
                let name = comp.as_os_str().to_string_lossy().to_string();
                let dir_entry = model::FileEntry {
                    name: name.clone(),
                    name_lower: name.to_ascii_lowercase(),
                    ext_lower: None,
                    path: Arc::new(anc_abs.clone()),
                    kind: model::EntryKind::Dir,
                };
                let mut node = TreeNode::new(
                    FileTreeItem::new(
                        dir_entry,
                        Vec::new(),
                        selected_paths.iter().any(|path| path == anc_abs.as_path()),
                    ),
                    anc_depth,
                    true,
                );

                if expanded_paths.contains(&anc_abs) {
                    node.expanded = true;
                    node.children_loaded = true;
                }
                nodes.push(node);
            }
        }

        let depth = components.len().saturating_sub(1);
        if entry.kind.is_dir()
            && let Some(existing_idx) = node_index_by_path.get(entry.path.as_ref()).copied()
        {
            if !highlights.is_empty() {
                nodes[existing_idx].item.highlights = highlights;
            }
            continue;
        }
        let idx = nodes.len();
        node_index_by_path.insert((*entry.path).clone(), idx);
        let mut node = TreeNode::new(
            FileTreeItem::new(
                entry.clone(),
                highlights,
                selected_paths
                    .iter()
                    .any(|path| path == entry.path.as_ref()),
            ),
            depth,
            entry.kind.is_dir(),
        );
        if entry.kind.is_dir() && expanded_paths.contains(entry.path.as_ref()) {
            node.expanded = true;
            node.children_loaded = true;
        }
        nodes.push(node);
    }

    if !cached_subtrees.is_empty() {
        let restore_indices = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                (node.has_children
                    && node.expanded
                    && node.children_loaded
                    && cached_subtrees.contains_key(node.item.entry.path.as_ref())
                    && nodes
                        .get(index + 1)
                        .is_none_or(|next| next.depth <= node.depth))
                .then_some(index)
            })
            .collect::<Vec<_>>();

        for index in restore_indices.into_iter().rev() {
            let path = (*nodes[index].item.entry.path).clone();
            if let Some(subtree) = cached_subtrees.get(&path) {
                nodes.splice(index + 1..index + 1, subtree.clone());
            }
        }
    }

    for i in 0..nodes.len() {
        if !nodes[i].has_children {
            continue;
        }
        let parent_depth = nodes[i].depth;
        let count = nodes[i + 1..]
            .iter()
            .take_while(|n| n.depth > parent_depth)
            .filter(|n| !n.has_children)
            .count();
        nodes[i].item.leaf_count = count;
    }

    nodes
}

fn tree_entry_key(entry: &model::FileEntry, root: &Path) -> Vec<(u8, String)> {
    let rel = entry
        .path
        .strip_prefix(root)
        .unwrap_or(entry.path.as_path());
    let parts = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return vec![(u8::from(!entry.kind.is_dir()), entry.name_lower.clone())];
    }

    let last = parts.len() - 1;
    parts
        .into_iter()
        .enumerate()
        .map(|(idx, part)| {
            let is_file = idx == last && !entry.kind.is_dir();
            (u8::from(is_file), part)
        })
        .collect()
}
