use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::widgets::components::select_list::{SelectItem, SelectItemView};
use crate::widgets::shared::list_policy;

#[derive(Clone)]
pub(super) enum ActiveOverlayItem {
    Parent,
    Entry { path: PathBuf, is_dir: bool },
}

impl FileBrowserComponent {
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

    fn overlay_list_options(
        &self,
        result: &ScanResult,
    ) -> (Vec<SelectItem>, Vec<ActiveOverlayItem>) {
        let mut options = Vec::with_capacity(
            result.options.len() + usize::from(self.should_show_parent_option()),
        );
        let mut items = Vec::with_capacity(
            result.entries.len() + usize::from(self.should_show_parent_option()),
        );

        if self.should_show_parent_option() {
            options.push(Self::parent_option_item());
            items.push(ActiveOverlayItem::Parent);
        }

        options.extend(result.options.clone());
        items.extend(result.entries.iter().map(|entry| ActiveOverlayItem::Entry {
            path: (*entry.path).clone(),
            is_dir: entry.kind.is_dir(),
        }));

        (options, items)
    }

    fn preferred_list_active_index(&self, items: &[ActiveOverlayItem]) -> Option<usize> {
        preferred_index_from_restore(
            self.pending_focus_restore.as_ref(),
            |path| preferred_list_item_pos(items, path),
            items.len(),
            items
                .iter()
                .position(|item| matches!(item, ActiveOverlayItem::Entry { .. })),
        )
    }

    fn set_preferred_tree_active(&mut self) {
        let Some(tree) = self.tree.as_mut() else {
            return;
        };
        let has_parent = tree
            .visible()
            .first()
            .and_then(|idx| tree.nodes().get(*idx))
            .is_some_and(|node| node.item.entry.name == "..");
        let preferred = preferred_index_from_restore(
            self.pending_focus_restore.as_ref(),
            |path| preferred_tree_visible_pos(tree, path),
            tree.visible().len(),
            has_parent.then_some(1),
        );
        if let Some(index) = preferred {
            tree.set_active_visible_index(index);
        }
    }

    fn submit_tree_build(&mut self, result: Arc<ScanResult>) {
        self.tree_build_seq = self.tree_build_seq.wrapping_add(1);
        self.tree_building = true;
        self.pending_tree_nodes = None;
        self.spinner_last_tick = crate::time::Instant::now();
        let expanded_paths = self.expanded_tree_paths();
        let cached_subtrees = self.expanded_tree_subtrees();
        self.tree_scanner
            .submit(super::tree_scanner::TreeBuildRequest {
                seq: self.tree_build_seq,
                browse_dir: self.browse_dir.clone(),
                show_parent_option: self.should_show_parent_option(),
                selected_paths: self.selected_paths.clone(),
                expanded_paths,
                cached_subtrees,
                result,
            });
    }

    fn clear_preferred_entry_state(&mut self) {
        self.pending_focus_restore = None;
    }

    fn apply_list_result(&mut self, result: &ScanResult) {
        if let Some(tokens) = self.pending_selection_tokens.clone() {
            self.selected_paths =
                self.resolve_tokens_against_result(tokens.as_slice(), Some(result));
        }
        let (options, items) = self.overlay_list_options(result);
        let preferred = self.preferred_list_active_index(items.as_slice());
        self.list.set_options(options);
        self.list_overlay_items = items;
        self.sync_list_selection();
        if let Some(index) = preferred {
            self.list.set_active_index(index);
        }
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
        self.sync_tree_selection();
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
            .is_some_and(|deadline| crate::time::Instant::now() < deadline)
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
        if let Some(tokens) = self.pending_selection_tokens.clone() {
            self.selected_paths =
                self.resolve_tokens_against_result(tokens.as_slice(), Some(result.as_ref()));
            if !self.selected_paths.is_empty() {
                self.pending_selection_tokens = None;
            }
        }

        if self.overlay_open {
            if self.browser_mode == BrowserMode::Tree {
                self.submit_tree_build(Arc::clone(&result));
            } else {
                self.apply_list_result(result.as_ref());
            }
        }

        self.last_scan_result = Some(result);
    }

    pub(super) fn active_list_item(&self) -> Option<ActiveOverlayItem> {
        self.list_overlay_items
            .get(self.list.active_index())
            .cloned()
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
                    if self.is_multi_select() {
                        self.toggle_selected_path(path);
                        self.sync_list_selection();
                        self.sync_tree_selection();
                        self.sync_multi_input_text(true);
                        self.schedule_scan();
                        return InteractionResult::handled();
                    }
                    self.text
                        .set_value(Value::Text(self.path_value_for_submit(path.as_path())));
                    return self.close_browser();
                }
                InteractionResult::ignored()
            }
        }
    }

    pub(super) fn reset_query_or_close_browser(&mut self) -> InteractionResult {
        let parsed = parse_input(&self.query_input(), &self.cwd);
        if !parsed.query.trim().is_empty() {
            if self.is_multi_select() {
                self.set_active_query(String::new());
                self.schedule_scan();
            } else {
                self.browse_into(self.browse_dir.clone());
            }
            return InteractionResult::handled();
        }
        self.close_browser()
    }

    pub(super) fn path_value_for_submit(&self, path: &Path) -> String {
        match self.value_mode {
            super::DisplayMode::Full => path.to_string_lossy().to_string(),
            super::DisplayMode::Relative => {
                if let Ok(rel) = path.strip_prefix(&self.cwd) {
                    rel.to_string_lossy().to_string()
                } else {
                    path.to_string_lossy().to_string()
                }
            }
            super::DisplayMode::Name => path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
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

fn preferred_list_item_pos(items: &[ActiveOverlayItem], pref_path: &Path) -> Option<usize> {
    if let Some(pos) = items.iter().position(|item| {
        matches!(
            item,
            ActiveOverlayItem::Entry { path, .. } if path.as_path() == pref_path
        )
    }) {
        return Some(pos);
    }

    let pref_name = pref_path.file_name()?.to_string_lossy();
    items.iter().position(|item| {
        matches!(
            item,
            ActiveOverlayItem::Entry { path, .. }
                if path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy() == pref_name)
        )
    })
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

fn preferred_index_from_restore(
    restore: Option<&FocusRestore>,
    by_path: impl FnOnce(&Path) -> Option<usize>,
    len: usize,
    first_real_index: Option<usize>,
) -> Option<usize> {
    match restore {
        Some(FocusRestore::History(memory)) => memory
            .path
            .as_deref()
            .and_then(by_path)
            .or(Some(list_policy::clamp_index(memory.index, len))),
        Some(FocusRestore::FirstRealEntry) => first_real_index,
        None => None,
    }
}
