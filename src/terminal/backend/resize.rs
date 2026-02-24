use super::*;

impl Terminal {
    pub(super) fn handle_inline_size_change(&mut self, old: TerminalSize, new: TerminalSize) {
        if self.mode != RenderMode::Inline {
            return;
        }
        let Some(inline) = self.inline_state.as_mut() else {
            return;
        };
        if old == new {
            return;
        }

        if new.height == 0 {
            inline.block_start_row = 0;
            inline.last_cursor_row = 0;
            inline.reanchor_after_resize = false;
            inline.last_resize_width_delta = 0;
            inline.last_resize_height_delta = 0;
            return;
        }

        let max_row = new.height.saturating_sub(1);
        inline.last_rendered_block_start_row = inline.last_rendered_block_start_row.min(max_row);
        let max_cursor_row = max_row.saturating_sub(inline.last_rendered_block_start_row);
        inline.last_cursor_row = inline.last_cursor_row.min(max_cursor_row);
        inline.last_cursor_col = inline.last_cursor_col.min(new.width.saturating_sub(1));
        inline.reanchor_after_resize = true;
        inline.last_resize_width_delta = new.width as i16 - old.width as i16;
        inline.last_resize_height_delta = new.height as i16 - old.height as i16;
    }

    pub(super) fn reanchor_inline_after_resize_if_needed(&mut self) {
        if self.mode != RenderMode::Inline {
            return;
        }
        if self.state.size.height == 0 {
            return;
        }
        let Some(inline_snapshot) = self.inline_state.as_ref().map(|inline| {
            (
                inline.reanchor_after_resize,
                inline.block_start_row,
                inline.last_rendered_block_start_row,
                inline.last_cursor_row,
                inline.last_resize_width_delta,
                inline.last_resize_height_delta,
                estimate_self_reflow_cursor_delta(inline, self.state.size.width),
            )
        }) else {
            return;
        };

        let (
            pending,
            block_start_row,
            last_rendered_block_start_row,
            last_cursor_row,
            width_delta,
            height_delta,
            self_reflow_delta,
        ) = inline_snapshot;
        if !pending {
            return;
        }

        let max_row = self.state.size.height.saturating_sub(1);
        let expected_cursor_row = last_rendered_block_start_row
            .saturating_add(last_cursor_row)
            .min(max_row);
        let maybe_actual_row = match position() {
            Ok((_, row)) => Some(row.min(max_row)),
            Err(_) => None,
        };

        let mut new_block_start_row = block_start_row;
        let mut new_last_rendered_block_start_row = last_rendered_block_start_row;
        if let Some(actual_row) = maybe_actual_row {
            let measured_delta = actual_row as i32 - expected_cursor_row as i32;
            let mut delta = measured_delta - self_reflow_delta;

            if height_delta == 0
                && ((width_delta > 0 && delta > 0) || (width_delta < 0 && delta < 0))
            {
                delta = 0;
            }
            if delta != 0 {
                new_block_start_row =
                    (block_start_row as i32 + delta).clamp(0, u16::MAX as i32) as u16;
                new_last_rendered_block_start_row =
                    (last_rendered_block_start_row as i32 + delta).clamp(0, max_row as i32) as u16;
            }
        }

        if let Some(inline) = self.inline_state.as_mut() {
            inline.reanchor_after_resize = false;
            inline.last_resize_width_delta = 0;
            inline.last_resize_height_delta = 0;
            inline.block_start_row = new_block_start_row;
            inline.last_rendered_block_start_row = new_last_rendered_block_start_row;
            let max_cursor_row = max_row.saturating_sub(new_last_rendered_block_start_row);
            inline.last_cursor_row = inline.last_cursor_row.min(max_cursor_row);
        }
    }
}
