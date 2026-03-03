use super::*;
use crate::runtime::event::WidgetAction;
use crate::terminal::{PointerButton, PointerEvent, PointerKind};

impl Repeater {
    fn handled_with_focus(&self) -> InteractionResult {
        let mut result = InteractionResult::handled();
        result.actions.push(WidgetAction::RequestFocus {
            target: self.base.id().to_string().into(),
        });
        result
    }

    fn pointer_field_at(&self, row: u16) -> Option<usize> {
        if !self.has_work() {
            return None;
        }
        let prefix_rows = self.line_prefix_rows() as u16;
        if row < prefix_rows {
            return None;
        }
        let body_row = row.saturating_sub(prefix_rows) as usize;
        match self.layout {
            RepeaterLayout::SingleField => (body_row == 0).then_some(self.active_field),
            RepeaterLayout::Stacked => (body_row < self.fields.len()).then_some(body_row),
        }
    }

    fn handle_pointer_left_down(&mut self, event: PointerEvent) -> InteractionResult {
        let Some(field_idx) = self.pointer_field_at(event.row) else {
            return InteractionResult::ignored();
        };
        self.active_field = field_idx;
        self.finished = false;
        self.handled_with_focus()
    }

    fn process_child_result(&mut self, mut result: InteractionResult) -> InteractionResult {
        let mut should_advance = false;
        result.actions.retain(|action| match action {
            WidgetAction::InputDone => {
                should_advance = true;
                false
            }
            _ => true,
        });

        if should_advance {
            result.merge(self.advance_cursor_and_submit_if_done());
        }
        if result.handled {
            result.request_render = true;
        }
        result
    }

    fn advance_cursor_and_submit_if_done(&mut self) -> InteractionResult {
        if !self.has_work() {
            return self.submit_or_done();
        }

        if self.finished {
            return self.submit_or_done();
        }

        if self.active_field + 1 < self.fields.len() {
            self.active_field += 1;
            return InteractionResult::handled();
        }

        if self.active_item + 1 < self.rows.len() {
            self.active_item += 1;
            self.active_field = 0;
            let mut result = InteractionResult::handled();
            if let Some(target) = &self.submit_target {
                result.actions.push(WidgetAction::ValueChanged {
                    change: ValueChange::with_target(
                        target.clone(),
                        self.build_committed_rows_value(),
                    ),
                });
            }
            return result;
        }

        self.finished = true;
        self.submit_or_done()
    }

    fn retreat_cursor(&mut self) -> InteractionResult {
        if !self.has_work() {
            return InteractionResult::ignored();
        }

        if self.finished {
            self.finished = false;
            return InteractionResult::handled();
        }

        if self.active_field > 0 {
            self.active_field -= 1;
            return InteractionResult::handled();
        }

        if self.active_item > 0 {
            self.active_item -= 1;
            self.active_field = self.fields.len().saturating_sub(1);
            return InteractionResult::handled();
        }

        InteractionResult::ignored()
    }

    fn submit_or_done(&self) -> InteractionResult {
        if let Some(target) = &self.submit_target {
            let mut result = InteractionResult::with_action(WidgetAction::ValueChanged {
                change: ValueChange::with_target(target.clone(), self.build_rows_value()),
            });
            result.actions.push(WidgetAction::InputDone);
            return result;
        }
        InteractionResult::input_done()
    }

    fn handle_group_key(&mut self, key: KeyEvent) -> InteractionResult {
        if !self.has_work() {
            return match key.code {
                KeyCode::Enter => self.submit_or_done(),
                _ => InteractionResult::ignored(),
            };
        }

        if let Some(widget) = self.active_field_widget_mut() {
            let result = widget.on_key(key);
            if result.handled {
                return self.process_child_result(result);
            }
        }

        match key.code {
            KeyCode::Enter | KeyCode::Tab => self.advance_cursor_and_submit_if_done(),
            KeyCode::BackTab => self.retreat_cursor(),
            _ => InteractionResult::ignored(),
        }
    }
}

impl Interactive for Repeater {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        self.handle_group_key(key)
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        match event.kind {
            PointerKind::Down(PointerButton::Left) => self.handle_pointer_left_down(event),
            _ => InteractionResult::ignored(),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let result = {
            let Some(widget) = self.active_field_widget_mut() else {
                return InteractionResult::ignored();
            };
            widget.on_text_action(action)
        };
        if !result.handled {
            return InteractionResult::ignored();
        }
        self.process_child_result(result)
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.active_field_widget_mut()?.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if !self.has_work() {
            return None;
        }
        let base = self.line_prefix_rows();
        let row = match self.layout {
            RepeaterLayout::SingleField => base,
            RepeaterLayout::Stacked => base.saturating_add(self.active_field),
        };
        let local = self
            .active_field_widget()
            .and_then(|widget| widget.cursor_pos())?;
        Some(CursorPos {
            col: local
                .col
                .saturating_add(self.active_field_label().len() as u16 + 4),
            row: row as u16 + local.row,
        })
    }

    fn value(&self) -> Option<Value> {
        Some(self.build_rows_value())
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::None => self.set_items(Vec::new()),
            Value::List(rows_or_items) => {
                if looks_like_rows_list(rows_or_items.as_slice(), self.fields.as_slice()) {
                    self.set_rows_value(rows_or_items.as_slice());
                } else {
                    self.set_items(rows_or_items);
                }
            }
            Value::Object(map) => {
                if let Some(Value::List(rows)) = map.get("rows") {
                    self.set_rows_value(rows.as_slice());
                } else {
                    self.set_items(Vec::new());
                }
            }
            scalar => self.set_items(vec![scalar]),
        }
        self.finished = false;
        self.clamp_cursor();
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (field_idx, widget) in row.fields.iter().enumerate() {
                if let Err(err) = widget.validate(mode) {
                    let field_label = self
                        .fields
                        .get(field_idx)
                        .map(|f| f.label.as_str())
                        .unwrap_or("field");
                    let item = self.item_label(row_idx);
                    return Err(format!(
                        "item {} [{}], {}: {}",
                        row_idx + 1,
                        item,
                        field_label,
                        err
                    ));
                }
            }
        }
        Ok(())
    }
}
