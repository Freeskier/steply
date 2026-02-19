use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::widgets::components::select_list::{SelectItem, SelectItemView};

#[derive(Clone)]
pub(super) enum ActiveOverlayItem {
    Parent,
    Entry { path: PathBuf, is_dir: bool },
}

impl FileBrowserInput {
    pub(super) fn should_show_parent_option(&self) -> bool {
        self.overlay_open && self.browse_dir.parent().is_some()
    }

    fn parent_option_item() -> SelectItem {
        SelectItem::new(
            Value::Text("..".to_string()),
            SelectItemView::Styled {
                text: "..".to_string(),
                highlights: vec![],
                style: crate::ui::style::Style::new()
                    .color(crate::ui::style::Color::Blue)
                    .bold(),
            },
        )
        .with_search_text("..")
    }

    fn overlay_list_options(&self, result: &ScanResult) -> (Vec<SelectItem>, bool) {
        if !self.should_show_parent_option() {
            return (result.options.clone(), false);
        }
        let mut options = Vec::with_capacity(result.options.len() + 1);
        options.push(Self::parent_option_item());
        options.extend(result.options.clone());
        (options, true)
    }

    fn set_preferred_list_active(&mut self, entries: &[model::FileEntry], has_parent: bool) {
        match self.pending_focus_restore.as_ref() {
            Some(FocusRestore::History(memory)) => {
                if let Some(pref_path) = memory.path.as_ref()
                    && let Some(pos) = preferred_entry_pos(entries, pref_path.as_path())
                {
                    self.list.set_active_index(pos + usize::from(has_parent));
                    return;
                }
                self.list.set_active_index(memory.index);
            }
            Some(FocusRestore::FirstRealEntry) => {
                if has_parent {
                    self.list.set_active_index(1);
                }
            }
            None => {}
        }
    }

    fn set_preferred_tree_active(&mut self) {
        let Some(tree) = self.tree.as_mut() else {
            return;
        };
        match self.pending_focus_restore.as_ref() {
            Some(FocusRestore::History(memory)) => {
                if let Some(pref_path) = memory.path.as_ref()
                    && let Some(pos) = preferred_tree_visible_pos(tree, pref_path.as_path())
                {
                    tree.set_active_visible_index(pos);
                    return;
                }
                tree.set_active_visible_index(memory.index);
            }
            Some(FocusRestore::FirstRealEntry) => {
                let has_parent = tree
                    .visible()
                    .first()
                    .and_then(|idx| tree.nodes().get(*idx))
                    .is_some_and(|node| node.item.entry.name == "..");
                if has_parent {
                    tree.set_active_visible_index(1);
                }
            }
            None => {}
        }
    }

    fn submit_tree_build(&mut self, result: Arc<ScanResult>) {
        self.tree_build_seq = self.tree_build_seq.wrapping_add(1);
        self.tree_building = true;
        self.pending_tree_nodes = None;
        self.spinner_last_tick = std::time::Instant::now();
        self.tree_scanner
            .submit(super::tree_scanner::TreeBuildRequest {
                seq: self.tree_build_seq,
                browse_dir: self.browse_dir.clone(),
                show_parent_option: self.should_show_parent_option(),
                result,
            });
    }

    fn clear_preferred_entry_state(&mut self) {
        self.pending_focus_restore = None;
    }

    fn apply_list_result(&mut self, result: &ScanResult) {
        let (options, has_parent_option) = self.overlay_list_options(result);
        self.list.set_options(options);
        self.set_preferred_list_active(result.entries.as_slice(), has_parent_option);
        self.tree_building = false;
        self.pending_tree_nodes = None;
        self.clear_preferred_entry_state();
    }

    fn apply_tree_nodes(&mut self, nodes: Vec<TreeNode<FileTreeItem>>) -> bool {
        self.tree_building = false;
        if !(self.overlay_open && self.browser_mode == BrowserMode::Tree) {
            return false;
        }
        if let Some(tree) = self.tree.as_mut() {
            tree.set_nodes(nodes);
        }
        self.set_preferred_tree_active();
        self.clear_preferred_entry_state();
        true
    }

    pub(super) fn poll_tree_build_results(&mut self) -> bool {
        let results = self.tree_scanner.try_recv_all();
        for result in results {
            if result.seq != self.tree_build_seq {
                continue;
            }
            self.pending_tree_nodes = Some((result.seq, result.nodes));
        }

        if self
            .debounce_deadline
            .is_some_and(|deadline| std::time::Instant::now() < deadline)
        {
            return false;
        }

        let Some((seq, nodes)) = self.pending_tree_nodes.take() else {
            return false;
        };
        if seq != self.tree_build_seq {
            return false;
        }

        self.apply_tree_nodes(nodes)
    }

    pub(super) fn apply_result(&mut self, result: Arc<ScanResult>) {
        self.scanning = false;
        self.text
            .set_completion_items(result.completion_items.clone());

        if self.overlay_open {
            if self.browser_mode == BrowserMode::Tree {
                self.submit_tree_build(Arc::clone(&result));
            } else {
                self.apply_list_result(result.as_ref());
            }
        }

        self.last_scan_result = Some(result);
    }

    fn has_dotdot(&self) -> bool {
        self.should_show_parent_option()
    }

    pub(super) fn active_list_item(&self) -> Option<ActiveOverlayItem> {
        let idx = self.list.active_index();
        if self.has_dotdot() && idx == 0 {
            return Some(ActiveOverlayItem::Parent);
        }
        let offset = usize::from(self.has_dotdot());
        let entries = self.last_scan_result.as_ref()?.entries.as_slice();
        idx.checked_sub(offset)
            .and_then(|entry_idx| entries.get(entry_idx))
            .map(|entry| ActiveOverlayItem::Entry {
                path: (*entry.path).clone(),
                is_dir: entry.kind.is_dir(),
            })
    }

    pub(super) fn active_tree_item(&self) -> Option<ActiveOverlayItem> {
        let node = self.tree.as_ref()?.active_node()?;
        if node.item.entry.name == ".." {
            Some(ActiveOverlayItem::Parent)
        } else {
            Some(ActiveOverlayItem::Entry {
                path: (*node.item.entry.path).clone(),
                is_dir: node.item.entry.kind.is_dir(),
            })
        }
    }

    pub(super) fn navigate_parent(&mut self) -> bool {
        let came_from = self.browse_dir.clone();
        let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) else {
            return false;
        };
        self.remember_active_focus_for_current_dir();
        let fallback = if self.focus_history.contains_key(&parent) {
            None
        } else {
            Some(FocusRestore::History(FocusMemory {
                index: 0,
                path: Some(came_from),
            }))
        };
        self.browse_into_with_restore(parent, fallback);
        true
    }

    pub(super) fn navigate_item(
        &mut self,
        item: ActiveOverlayItem,
        allow_file_select: bool,
    ) -> InteractionResult {
        match item {
            ActiveOverlayItem::Parent => {
                let _ = self.navigate_parent();
                InteractionResult::handled()
            }
            ActiveOverlayItem::Entry { path, is_dir } => {
                if is_dir {
                    self.remember_active_focus_for_current_dir();
                    self.browse_into(path);
                    return InteractionResult::handled();
                }
                if allow_file_select {
                    self.text
                        .set_value(Value::Text(self.path_value_for_submit(path.as_path())));
                    return self.close_browser();
                }
                InteractionResult::ignored()
            }
        }
    }

    pub(super) fn reset_query_or_close_browser(&mut self) -> InteractionResult {
        let parsed = parse_input(&self.current_input(), &self.cwd);
        if !parsed.query.trim().is_empty() {
            self.browse_into(self.browse_dir.clone());
            return InteractionResult::handled();
        }
        self.close_browser()
    }

    pub(super) fn path_value_for_submit(&self, path: &Path) -> String {
        if let Ok(rel) = path.strip_prefix(&self.cwd) {
            rel.to_string_lossy().to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    }

    fn remember_active_focus_for_current_dir(&mut self) {
        let memory = if self.browser_mode == BrowserMode::Tree {
            self.tree
                .as_ref()
                .map(|tree| {
                    let index = tree.active_visible_index();
                    let path = tree.active_node().and_then(|node| {
                        (node.item.entry.name != "..").then(|| (*node.item.entry.path).clone())
                    });
                    FocusMemory { index, path }
                })
                .unwrap_or(FocusMemory {
                    index: 0,
                    path: None,
                })
        } else {
            let index = self.list.active_index();
            let path = match self.active_list_item() {
                Some(ActiveOverlayItem::Entry { path, .. }) => Some(path),
                _ => None,
            };
            FocusMemory { index, path }
        };
        self.focus_history.insert(self.browse_dir.clone(), memory);
    }
}

fn preferred_entry_pos(entries: &[model::FileEntry], pref_path: &Path) -> Option<usize> {
    if let Some(pos) = entries
        .iter()
        .position(|entry| entry.path.as_ref() == pref_path)
    {
        return Some(pos);
    }

    let pref_name = pref_path.file_name()?.to_string_lossy();
    entries.iter().position(|entry| entry.name == pref_name)
}

fn preferred_tree_visible_pos(tree: &TreeView<FileTreeItem>, pref_path: &Path) -> Option<usize> {
    if let Some(pos) = tree
        .visible()
        .iter()
        .position(|&idx| tree.nodes()[idx].item.entry.path.as_ref() == pref_path)
    {
        return Some(pos);
    }

    let pref_name = pref_path.file_name()?.to_string_lossy();
    tree.visible()
        .iter()
        .position(|&idx| tree.nodes()[idx].item.entry.name == pref_name)
}
