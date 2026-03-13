use super::overlay_interaction::ActiveOverlayItem;
use super::*;
use crate::widgets::components::tree_view::TreeNode;
use crate::widgets::shared::keymap;

impl FileBrowserComponent {
    fn toggle_active_selection(&mut self) -> InteractionResult {
        let Some(item) = self.active_item_in_mode(self.browser_mode) else {
            return InteractionResult::handled();
        };
        let ActiveOverlayItem::Entry { path, is_dir } = item else {
            return InteractionResult::handled();
        };
        if is_dir {
            return InteractionResult::handled();
        }
        self.toggle_selected_path(path);
        self.sync_list_selection();
        self.sync_tree_selection();
        self.sync_multi_input_text(true);
        self.schedule_scan();
        InteractionResult::handled()
    }

    fn set_browser_mode(&mut self, mode: BrowserMode) {
        if self.browser_mode == mode {
            return;
        }
        self.browser_mode = mode;
        if self.browser_mode == BrowserMode::Tree {
            self.ensure_tree_widget();
        }
        self.refresh_overlay_from_last_result();
    }

    fn refresh_overlay_from_last_result(&mut self) {
        if let Some(result) = self.last_scan_result.clone() {
            self.apply_result(result);
        }
    }

    fn active_item_in_mode(&self, mode: BrowserMode) -> Option<ActiveOverlayItem> {
        match mode {
            BrowserMode::List => self.active_list_item(),
            BrowserMode::Tree => self.active_tree_item(),
        }
    }

    fn navigate_active_item(
        &mut self,
        mode: BrowserMode,
        allow_file_select: bool,
    ) -> InteractionResult {
        let Some(item) = self.active_item_in_mode(mode) else {
            return InteractionResult::handled();
        };
        self.navigate_item(item, allow_file_select)
    }

    fn move_tree_active(&mut self, delta: isize) -> bool {
        self.tree
            .as_mut()
            .map(|tree| tree.move_active(delta))
            .unwrap_or(false)
    }

    pub(super) fn handle_browser_key(&mut self, key: KeyEvent) -> InteractionResult {
        if keymap::is_ctrl_char(key, 't') {
            let next = match self.browser_mode {
                BrowserMode::List => BrowserMode::Tree,
                BrowserMode::Tree => BrowserMode::List,
            };
            self.set_browser_mode(next);
            return InteractionResult::handled();
        }

        if self.browser_mode == BrowserMode::Tree {
            return self.handle_tree_key(key);
        }

        match key.code {
            KeyCode::Esc => self.reset_query_or_close_browser(),
            KeyCode::Enter => {
                if self.is_multi_select() {
                    self.close_browser()
                } else {
                    self.navigate_active_item(BrowserMode::List, true)
                }
            }
            KeyCode::Right => {
                let Some(item) = self.active_item_in_mode(BrowserMode::List) else {
                    return InteractionResult::handled();
                };
                if matches!(item, ActiveOverlayItem::Parent) {
                    InteractionResult::handled()
                } else {
                    self.navigate_item(item, false)
                }
            }
            KeyCode::Left => InteractionResult::handled_if(self.navigate_parent()),

            KeyCode::Up | KeyCode::Down => self.list.on_key(key),
            KeyCode::Char(' ') if self.is_multi_select() => self.toggle_active_selection(),

            _ => self.handle_text_key_with_rescan(key),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => self.reset_query_or_close_browser(),
            KeyCode::Enter if self.is_multi_select() => self.close_browser(),

            KeyCode::Char(' ')
                if self.is_multi_select()
                    && keymap::has_exact_modifiers(key, crate::terminal::KeyModifiers::CONTROL) =>
            {
                self.toggle_active_selection()
            }

            KeyCode::Up => InteractionResult::handled_if(self.move_tree_active(-1)),
            KeyCode::Down => InteractionResult::handled_if(self.move_tree_active(1)),

            KeyCode::Right => {
                let Some(item) = self.active_item_in_mode(BrowserMode::Tree) else {
                    return InteractionResult::handled();
                };
                if matches!(item, ActiveOverlayItem::Parent) {
                    InteractionResult::handled()
                } else {
                    self.navigate_item(item, false)
                }
            }
            KeyCode::Enter => self.navigate_active_item(BrowserMode::Tree, true),

            KeyCode::Char(' ') => {
                if self.is_multi_select()
                    && self.active_tree_item().is_some_and(|item| {
                        matches!(item, ActiveOverlayItem::Entry { is_dir: false, .. })
                    })
                {
                    return self.toggle_active_selection();
                }
                if matches!(self.active_tree_item(), Some(ActiveOverlayItem::Parent)) {
                    let _ = self.navigate_parent();
                    return InteractionResult::handled();
                }

                let active = self.tree.as_ref().and_then(|tree| {
                    let node = tree.active_node()?;
                    let idx = tree.active_node_idx()?;
                    Some((
                        idx,
                        node.has_children,
                        node.children_loaded,
                        node.expanded,
                        node.item.entry.path.clone(),
                    ))
                });
                let Some((node_idx, has_children, children_loaded, expanded, path)) = active else {
                    return InteractionResult::handled();
                };

                if !has_children {
                    return InteractionResult::handled();
                }

                if expanded {
                    if let Some(tree) = self.tree.as_mut() {
                        tree.collapse_active();
                    }
                    return InteractionResult::handled();
                }

                if !children_loaded {
                    let child_entries = filter_entries(
                        list_dir(path.as_ref(), self.hide_hidden),
                        self.entry_filter,
                        self.ext_filter.as_ref(),
                    );
                    let children = child_entries
                        .into_iter()
                        .map(|entry| {
                            let is_dir = entry.kind.is_dir();
                            TreeNode::new(
                                FileTreeItem::new(
                                    entry.clone(),
                                    Vec::new(),
                                    self.is_selected_path(entry.path.as_ref()),
                                ),
                                0,
                                is_dir,
                            )
                        })
                        .collect::<Vec<_>>();
                    if let Some(tree) = self.tree.as_mut() {
                        tree.insert_children_after(node_idx, children);
                    }
                } else if let Some(tree) = self.tree.as_mut() {
                    tree.expand_active();
                }
                InteractionResult::handled()
            }

            KeyCode::Left => {
                let _ = self.navigate_parent();
                InteractionResult::handled()
            }

            _ => self.handle_text_key_with_rescan(key),
        }
    }
}

#[cfg(test)]
#[path = "../tests/file_browser_interaction.rs"]
mod tests;
