use super::*;

impl Interactive for ObjectEditor {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }

        if self.filter_focus {
            return self.handle_filter_key(key);
        }

        match &self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::EditValue { .. } => self.handle_edit_value(key),
            Mode::EditKey { .. } => self.handle_edit_key(key),
            Mode::InsertType { .. } => self.handle_insert_type(key),
            Mode::InsertValue { .. } => self.handle_insert_value(key),
            Mode::ConfirmDelete { .. } => self.handle_confirm_delete(key),
            Mode::Move { .. } => self.handle_move(key),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(self.value.clone())
    }

    fn set_value(&mut self, value: Value) {
        self.value = value;
        self.expanded.clear();
        self.array_item_names.clear();
        self.expand_all_top_level();
        self.rebuild();
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        if mode == ValidationMode::Submit
            && let Some(error) = self.pending_insert_value_error()
        {
            return Err(error);
        }
        Ok(())
    }
    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.filter_focus {
            let local = self.filter.cursor_pos()?;
            let mut row: u16 = 0;
            if !self.base.label().is_empty() {
                row = row.saturating_add(1);
            }
            return Some(CursorPos {
                col: local.col.saturating_add(8),
                row,
            });
        }
        let header_rows = self.headers_row_offset();
        let (start, end) = self.tree.visible_range();
        let tree_lines = self.tree.render_lines(true);
        match &self.mode {
            Mode::EditKey { vis, key_value } | Mode::EditValue { vis, key_value } => {
                if *vis < start || *vis >= end {
                    return None;
                }
                let line_idx = *vis - start;
                let tree_line = tree_lines.get(line_idx)?;
                let prefix_col = Self::tree_prefix_width(tree_line);
                let local = key_value.cursor_pos()?;
                Some(CursorPos {
                    col: prefix_col.saturating_add(local.col),
                    row: header_rows.saturating_add(line_idx as u16),
                })
            }
            Mode::InsertType {
                after_vis,
                key_value,
            }
            | Mode::InsertValue {
                after_vis,
                key_value,
                ..
            } => {
                if *after_vis < start || *after_vis >= end {
                    return None;
                }
                let line_idx = *after_vis - start;
                let tree_line = tree_lines.get(line_idx)?;
                let prefix_col = Self::tree_prefix_width(tree_line);
                let inline_on_placeholder = self.is_placeholder_vis(*after_vis);
                let local = key_value.cursor_pos()?;
                Some(CursorPos {
                    col: prefix_col.saturating_add(local.col),
                    row: header_rows
                        .saturating_add(line_idx as u16)
                        .saturating_add(if inline_on_placeholder { 0 } else { 1 }),
                })
            }
            _ => None,
        }
    }

    fn on_text_action(&mut self, action: crate::widgets::traits::TextAction) -> InteractionResult {
        if self.filter_focus {
            let before = self.filter_query();
            let mut result = self.filter.on_text_action(action);
            result
                .actions
                .retain(|a| !matches!(a, crate::runtime::event::WidgetAction::InputDone));
            if self.filter_query() != before {
                self.apply_filter_from_input();
                return InteractionResult::handled();
            }
            return result;
        }

        match &mut self.mode {
            Mode::EditValue { key_value, .. }
            | Mode::EditKey { key_value, .. }
            | Mode::InsertType { key_value, .. }
            | Mode::InsertValue { key_value, .. } => key_value.on_text_action(action),
            _ => InteractionResult::ignored(),
        }
    }

    fn completion(&mut self) -> Option<crate::widgets::traits::CompletionState<'_>> {
        if !self.filter_focus {
            return None;
        }
        self.filter.completion()
    }
}

impl ObjectEditor {
    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.filter_focus = false;
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.filter_focus = false;
                self.tree.move_active(1);
                InteractionResult::handled()
            }
            _ => {
                let before = self.filter_query();
                let mut result = self.filter.on_key(key);
                result
                    .actions
                    .retain(|a| !matches!(a, crate::runtime::event::WidgetAction::InputDone));
                if self.filter_query() != before {
                    self.apply_filter_from_input();
                    return InteractionResult::handled();
                }
                result
            }
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }
        match key.code {
            KeyCode::Up => {
                self.tree.move_active(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.tree.move_active(1);
                InteractionResult::handled()
            }
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Left => {
                self.toggle_expand();
                InteractionResult::handled()
            }
            KeyCode::Char('e') => {
                self.start_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Char('r') => {
                self.start_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Char('i') => {
                self.start_insert();
                InteractionResult::handled()
            }
            KeyCode::Char('d') => {
                self.start_delete();
                InteractionResult::handled()
            }
            KeyCode::Char('m') => {
                self.start_move();
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::input_done(),
            _ => InteractionResult::ignored(),
        }
    }

    fn handle_edit_value(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                self.commit_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Tab => {
                self.commit_edit_value();
                self.start_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            _ => {
                if let Mode::EditValue { key_value, .. } = &mut self.mode {
                    key_value.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                self.commit_edit_key();
                InteractionResult::handled()
            }
            KeyCode::Tab => {
                self.commit_edit_key();
                self.start_edit_value();
                InteractionResult::handled()
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            _ => {
                if let Mode::EditKey { key_value, .. } = &mut self.mode {
                    key_value.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_type(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                self.commit_insert_type();
                InteractionResult::handled()
            }
            _ => {
                if let Mode::InsertType { key_value, .. } = &mut self.mode {
                    key_value.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_value(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                if self.pending_insert_value_error().is_some() {
                    return InteractionResult::with_action(
                        crate::runtime::event::WidgetAction::ValidateFocusedSubmit,
                    );
                }
                self.commit_insert_value();
                InteractionResult::handled()
            }
            _ => {
                if let Mode::InsertValue { key_value, .. } = &mut self.mode {
                    key_value.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                let confirmed = if let Mode::ConfirmDelete { select, .. } = &self.mode {
                    select
                        .value()
                        .and_then(|v| v.to_text_scalar())
                        .map(|s| s == "Yes")
                        .unwrap_or(false)
                } else {
                    false
                };
                self.commit_delete(confirmed);
                InteractionResult::handled()
            }
            _ => {
                if let Mode::ConfirmDelete { select, .. } = &mut self.mode {
                    select.on_key(key);
                }
                InteractionResult::handled()
            }
        }
    }

    fn handle_move(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc | KeyCode::Char('m') => {
                self.mode = Mode::Normal;
                InteractionResult::handled()
            }
            KeyCode::Up => {
                self.move_node(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.move_node(1);
                InteractionResult::handled()
            }
            _ => InteractionResult::handled(),
        }
    }
}
