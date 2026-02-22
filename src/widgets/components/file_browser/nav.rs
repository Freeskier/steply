use super::overlay::ActiveOverlayItem;
use super::*;
use crate::widgets::components::tree_view::TreeNode;

impl FileBrowserInput {
    pub(super) fn handle_browser_key(&mut self, key: KeyEvent) -> InteractionResult {

        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.browser_mode = match self.browser_mode {
                BrowserMode::List => BrowserMode::Tree,
                BrowserMode::Tree => BrowserMode::List,
            };
            if self.browser_mode == BrowserMode::Tree {
                self.ensure_tree_widget();

                if let Some(result) = self.last_scan_result.clone() {
                    self.apply_result(result);
                }
            } else if let Some(result) = self.last_scan_result.clone() {

                self.apply_result(result);
            }
            return InteractionResult::handled();
        }


        if self.browser_mode == BrowserMode::Tree {
            return self.handle_tree_key(key);
        }

        match key.code {
            KeyCode::Esc => self.reset_query_or_close_browser(),
            KeyCode::Enter => {
                let Some(item) = self.active_list_item() else {
                    return InteractionResult::handled();
                };
                self.navigate_item(item, true)
            }
            KeyCode::Right => {
                let Some(item) = self.active_list_item() else {
                    return InteractionResult::ignored();
                };
                if matches!(item, ActiveOverlayItem::Parent) {
                    InteractionResult::handled()
                } else {
                    self.navigate_item(item, false)
                }
            }
            KeyCode::Left => {
                if self.navigate_parent() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }

            KeyCode::Up | KeyCode::Down => self.list.on_key(key),


            _ => self.handle_text_key_with_rescan(key),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => self.reset_query_or_close_browser(),

            KeyCode::Up => {
                if self
                    .tree
                    .as_mut()
                    .map(|t| t.move_active(-1))
                    .unwrap_or(false)
                {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Down => {
                if self
                    .tree
                    .as_mut()
                    .map(|t| t.move_active(1))
                    .unwrap_or(false)
                {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }

            KeyCode::Right => {
                let Some(item) = self.active_tree_item() else {
                    return InteractionResult::handled();
                };
                if matches!(item, ActiveOverlayItem::Parent) {
                    InteractionResult::handled()
                } else {
                    self.navigate_item(item, false)
                }
            }
            KeyCode::Enter => {
                let Some(item) = self.active_tree_item() else {
                    return InteractionResult::handled();
                };
                self.navigate_item(item, true)
            }

            KeyCode::Char(' ') => {
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
                            TreeNode::new(FileTreeItem::new(entry, Vec::new()), 0, is_dir)
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
