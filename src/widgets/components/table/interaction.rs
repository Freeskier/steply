use super::*;
use crate::runtime::event::WidgetAction;
use crate::terminal::{PointerButton, PointerEvent, PointerKind};
use crate::widgets::shared::list_core;

impl Table {
    fn handled_with_focus(&self) -> InteractionResult {
        let mut result = InteractionResult::handled();
        result.actions.push(WidgetAction::RequestFocus {
            target: self.base.id().to_string().into(),
        });
        result
    }

    fn focus_next_column(&mut self) -> InteractionResult {
        let Some(next) = list_core::cycle_next(self.active_col, self.columns.len()) else {
            return InteractionResult::ignored();
        };
        self.active_col = next;
        InteractionResult::handled()
    }

    fn focus_prev_column(&mut self) -> InteractionResult {
        let Some(prev) = list_core::cycle_prev(self.active_col, self.columns.len()) else {
            return InteractionResult::ignored();
        };
        self.active_col = prev;
        InteractionResult::handled()
    }

    fn pointer_filter_row(&self) -> Option<u16> {
        if !self.filter.is_visible() {
            return None;
        }
        let label_rows = if self.base.label().is_empty() { 0 } else { 1 };
        Some(label_rows)
    }

    fn pointer_header_row(&self) -> u16 {
        let label_rows = if self.base.label().is_empty() { 0 } else { 1 };
        let filter_rows = if self.filter.is_visible() { 1 } else { 0 };
        match self.style {
            TableStyle::Grid => label_rows + filter_rows + 1,
            TableStyle::Clean => label_rows + filter_rows,
        }
    }

    fn pointer_column_at(&self, col: u16) -> Option<usize> {
        let col_widths = self.compute_column_widths(&self.fallback_context());
        let starts = self.body_col_starts(col_widths.as_slice());
        let mut selected = None;
        for (col_idx, start) in starts.iter().copied().enumerate() {
            if col < start {
                break;
            }
            selected = Some(col_idx);
        }
        selected
    }

    fn handle_pointer_left_down(&mut self, event: PointerEvent) -> InteractionResult {
        self.clamp_focus();

        if self
            .pointer_filter_row()
            .is_some_and(|filter_row| filter_row == event.row)
        {
            self.filter.set_focused(true);
            self.edit_mode = false;
            self.move_mode = false;
            return self.handled_with_focus();
        }

        self.filter.set_focused(false);
        let Some(col_idx) = self.pointer_column_at(event.col) else {
            return InteractionResult::ignored();
        };

        if event.row == self.pointer_header_row() {
            self.focus = TableFocus::Header;
            self.active_col = col_idx;
            self.edit_mode = false;
            self.move_mode = false;
            return self.handled_with_focus();
        }

        let body_start = self.body_row_start();
        if event.row < body_start {
            return InteractionResult::ignored();
        }
        let visible_pos = event.row.saturating_sub(body_start) as usize;
        let Some(row_idx) = self.visible_rows.get(visible_pos).copied() else {
            return InteractionResult::ignored();
        };
        self.focus = TableFocus::Body;
        self.active_row = row_idx;
        self.active_col = col_idx;
        self.move_mode = false;
        self.edit_mode = true;
        self.handled_with_focus()
    }

    fn on_key_header(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char('i') => {
                if self.rows.is_empty() {
                    self.add_row();
                } else {
                    self.focus = TableFocus::Body;
                    self.active_row = self.rows.len().saturating_sub(1);
                    self.insert_row_after_active();
                }
                self.edit_mode = true;
                InteractionResult::handled()
            }
            KeyCode::Tab | KeyCode::Right => self.focus_next_column(),
            KeyCode::BackTab | KeyCode::Left => self.focus_prev_column(),
            KeyCode::Down => {
                if !self.visible_rows.is_empty() {
                    self.focus = TableFocus::Body;
                    self.active_row = self.visible_rows.first().copied().unwrap_or(0);
                    self.edit_mode = false;
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_sort(self.active_col);
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_key_body(&mut self, key: KeyEvent) -> InteractionResult {
        if self.move_mode {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('m') => {
                    self.move_mode = false;
                    InteractionResult::handled()
                }
                KeyCode::Up => {
                    let moved = self.move_active_row_by(-1);
                    if moved {
                        InteractionResult::handled()
                    } else {
                        InteractionResult::ignored()
                    }
                }
                KeyCode::Down => {
                    let moved = self.move_active_row_by(1);
                    if moved {
                        InteractionResult::handled()
                    } else {
                        InteractionResult::ignored()
                    }
                }
                _ => InteractionResult::handled(),
            };
        }

        if !self.edit_mode {
            return match key.code {
                KeyCode::Char('i') => {
                    self.insert_row_after_active();
                    InteractionResult::handled()
                }
                KeyCode::Char('d') => {
                    self.delete_active_row();
                    InteractionResult::handled()
                }
                KeyCode::Char('m') => {
                    self.move_mode = self.rows.len() > 1;
                    self.sort = None;
                    self.edit_mode = false;
                    InteractionResult::handled()
                }
                KeyCode::Char('e') => {
                    self.edit_mode = true;
                    InteractionResult::handled()
                }
                KeyCode::Up => {
                    if !self.move_active_visible(-1) {
                        self.focus = TableFocus::Header;
                    }
                    InteractionResult::handled()
                }
                KeyCode::Down => {
                    let _ = self.move_active_visible(1);
                    InteractionResult::handled()
                }
                KeyCode::Tab => self.focus_next_column(),
                KeyCode::BackTab => self.focus_prev_column(),
                KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.focus_prev_column()
                }
                KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.focus_next_column()
                }
                _ => InteractionResult::ignored(),
            };
        }

        match key.code {
            KeyCode::Esc => {
                self.edit_mode = false;
                return InteractionResult::handled();
            }
            KeyCode::Enter => {
                self.edit_mode = false;
                return InteractionResult::handled();
            }
            KeyCode::Tab => {
                return self.focus_next_column();
            }
            KeyCode::BackTab => {
                return self.focus_prev_column();
            }
            _ => {}
        }

        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        let result = filter_utils::sanitize_interaction_result(cell.on_key(key));
        if result.handled {
            self.apply_filter(self.active_row_id());
        }
        result
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        match self
            .filter
            .handle_key_with_change(key, filter_utils::FilterEscBehavior::Hide)
        {
            filter_utils::FilterKeyOutcome::Ignored => InteractionResult::ignored(),
            filter_utils::FilterKeyOutcome::Hide => {
                self.toggle_filter_visibility();
                InteractionResult::handled()
            }
            filter_utils::FilterKeyOutcome::Blur => {
                self.filter.set_focused(false);
                InteractionResult::handled()
            }
            filter_utils::FilterKeyOutcome::Edited(outcome) => {
                outcome.refresh_if_changed(|| self.apply_filter(self.active_row_id()))
            }
        }
    }
}

impl Interactive for Table {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }
        if self.filter.is_focused() {
            return self.handle_filter_key(key);
        }
        if key.modifiers == KeyModifiers::NONE
            && key.code == KeyCode::Enter
            && !self.edit_mode
            && !self.move_mode
        {
            return InteractionResult::input_done();
        }
        self.clamp_focus();
        match self.focus {
            TableFocus::Header => self.on_key_header(key),
            TableFocus::Body => self.on_key_body(key),
        }
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        match event.kind {
            PointerKind::Down(PointerButton::Left) => self.handle_pointer_left_down(event),
            _ => InteractionResult::ignored(),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.filter.is_focused() {
            return self
                .filter
                .handle_text_action_with_change(action)
                .refresh_if_changed(|| self.apply_filter(self.active_row_id()));
        }
        if self.focus != TableFocus::Body || !self.edit_mode {
            return InteractionResult::ignored();
        }
        let Some(cell) = self.active_cell_mut() else {
            return InteractionResult::ignored();
        };
        let result = filter_utils::sanitize_interaction_result(cell.on_text_action(action));
        if result.handled {
            self.apply_filter(self.active_row_id());
        }
        result
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if self.filter.is_focused() {
            return self.filter.completion();
        }
        if self.focus != TableFocus::Body || !self.edit_mode {
            return None;
        }
        self.active_cell_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.filter.is_focused() {
            let local = self.filter.cursor_pos()?;
            let row = if self.base.label().is_empty() { 0 } else { 1 };
            return Some(CursorPos {
                col: local.col.saturating_add(8),
                row,
            });
        }
        if self.focus != TableFocus::Body {
            return None;
        }
        if !self.edit_mode {
            let row_offset = self.active_visible_pos()? as u16;
            let row = self.body_row_start().saturating_add(row_offset);
            return Some(cursor_anchor::anchored_cursor(row as usize, 0));
        }
        let local = self.active_cell().and_then(|cell| cell.cursor_pos())?;

        let col_widths = self.compute_column_widths(&self.fallback_context());
        let col_starts = self.body_col_starts(col_widths.as_slice());
        let marker_offset = if !self.show_row_numbers && self.active_col == 0 {
            2
        } else {
            0
        };
        let col = col_starts
            .get(self.active_col)
            .copied()
            .unwrap_or_default()
            .saturating_add(marker_offset)
            .saturating_add(local.col);
        let row_offset = self.active_visible_pos().unwrap_or(0) as u16;
        let row = self
            .body_row_start()
            .saturating_add(row_offset)
            .saturating_add(local.row);
        Some(CursorPos { col, row })
    }

    fn cursor_visible(&self) -> bool {
        cursor_anchor::visible_when_text_cursor(
            self.filter.is_focused() || (self.focus == TableFocus::Body && self.edit_mode),
        )
    }

    fn value(&self) -> Option<Value> {
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let mut map = IndexMap::<String, Value>::new();
                for (col_idx, col) in self.columns.iter().enumerate() {
                    let value = row
                        .cells
                        .get(col_idx)
                        .and_then(|cell| cell.value())
                        .unwrap_or(Value::None);
                    map.insert(col.key.clone(), value);
                }
                Value::Object(map)
            })
            .collect::<Vec<_>>();
        Some(Value::List(rows))
    }

    fn set_value(&mut self, value: Value) {
        self.rows.clear();
        match value {
            Value::None => {}
            Value::List(list) => {
                for entry in list {
                    let row_id = self.next_row_id;
                    self.next_row_id = self.next_row_id.saturating_add(1);
                    self.rows.push(self.build_row(row_id, Some(&entry)));
                }
            }
            other => {
                let row_id = self.next_row_id;
                self.next_row_id = self.next_row_id.saturating_add(1);
                self.rows.push(self.build_row(row_id, Some(&other)));
            }
        }

        self.clamp_focus();
        self.apply_sort_preserving_focus(self.active_row_id());
        if self.rows.is_empty() {
            self.focus = TableFocus::Header;
        } else if self.focus == TableFocus::Header {
            self.focus = TableFocus::Body;
            self.active_row = 0;
        }
        self.move_mode = false;
        self.edit_mode = false;
        self.apply_filter(self.active_row_id());
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                if let Err(error) = cell.validate(mode) {
                    let header = self
                        .columns
                        .get(col_idx)
                        .map(|col| col.header.as_str())
                        .unwrap_or("column");
                    return Err(format!("row {}, {}: {}", row_idx + 1, header, error));
                }
            }
        }
        Ok(())
    }
}
