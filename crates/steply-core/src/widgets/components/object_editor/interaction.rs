use super::*;
use crate::widgets::shared::keymap;

impl Interactive for ObjectEditor {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if let Some(outcome) = self.filter.handle_toggle_shortcut(key) {
            if outcome.hidden {
                self.tree.clear_filter();
                return InteractionResult::handled();
            }
            return outcome.refresh_if_changed(|| self.apply_filter_from_input());
        }

        if self.filter.is_focused() {
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
        Some(self.draft_value())
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
        if self.filter.is_focused() {
            let mut row: u16 = 0;
            if !self.base.label().is_empty() {
                row = row.saturating_add(1);
            }
            return self.filter.anchored_cursor_pos(row);
        }
        let header_rows = self.headers_row_offset();
        let (start, end) = self.tree.visible_range();
        let tree_lines = self.tree.render_lines(true);
        match &self.mode {
            Mode::EditKey {
                visible_index,
                key_value,
            }
            | Mode::EditValue {
                visible_index,
                key_value,
            } => {
                if *visible_index < start || *visible_index >= end {
                    return None;
                }
                let line_idx = *visible_index - start;
                let tree_line = tree_lines.get(line_idx)?;
                let prefix_col = Self::tree_prefix_width(tree_line);
                let local = key_value.cursor_pos()?;
                Some(CursorPos {
                    col: prefix_col.saturating_add(local.col),
                    row: header_rows.saturating_add(line_idx as u16),
                })
            }
            Mode::InsertType {
                after_visible_index,
                key_value,
            }
            | Mode::InsertValue {
                after_visible_index,
                key_value,
                ..
            } => {
                if *after_visible_index < start || *after_visible_index >= end {
                    return None;
                }
                let line_idx = *after_visible_index - start;
                let tree_line = tree_lines.get(line_idx)?;
                let prefix_col = Self::tree_prefix_width(tree_line);
                let inline_on_placeholder = self.is_placeholder_visible_index(*after_visible_index);
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
        if self.filter.is_focused() {
            return self
                .filter
                .handle_text_action(action)
                .refresh_if_changed(|| self.apply_filter_from_input());
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
        if !self.filter.is_focused() {
            return None;
        }
        self.filter.completion()
    }
}

impl ObjectEditor {
    fn back_to_normal_mode(&mut self) -> InteractionResult {
        self.mode = Mode::Normal;
        InteractionResult::handled()
    }

    fn forward_mode_key(&mut self, key: KeyEvent) {
        match &mut self.mode {
            Mode::EditValue { key_value, .. }
            | Mode::EditKey { key_value, .. }
            | Mode::InsertType { key_value, .. }
            | Mode::InsertValue { key_value, .. } => key_value.on_key(key),
            Mode::ConfirmDelete { select, .. } => {
                let _ = select.on_key(key);
            }
            Mode::Normal | Mode::Move { .. } => {}
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        let outcome = self.filter.handle_key(key);
        if outcome.blurred && key.code == KeyCode::Down {
            self.tree.move_active(1);
        }
        outcome.refresh_if_changed(|| self.apply_filter_from_input())
    }

    fn handle_normal(&mut self, key: KeyEvent) -> InteractionResult {
        if !keymap::has_no_modifiers(key) {
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
                if let Some(obj) = self.active_obj() {
                    if !obj.is_index
                        && !obj.is_placeholder
                        && matches!(obj.value, Value::Object(_) | Value::List(_))
                    {
                        self.start_edit_key();
                    } else {
                        self.start_edit_value();
                    }
                }
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
            KeyCode::Esc => self.back_to_normal_mode(),
            _ => {
                self.forward_mode_key(key);
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
            KeyCode::Esc => self.back_to_normal_mode(),
            _ => {
                self.forward_mode_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_type(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => self.back_to_normal_mode(),
            KeyCode::Enter => {
                self.commit_insert_type();
                InteractionResult::handled()
            }
            _ => {
                self.forward_mode_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_insert_value(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => self.back_to_normal_mode(),
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
                self.forward_mode_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => self.back_to_normal_mode(),
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
                self.forward_mode_key(key);
                InteractionResult::handled()
            }
        }
    }

    fn handle_move(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc | KeyCode::Char('m') => self.back_to_normal_mode(),
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
