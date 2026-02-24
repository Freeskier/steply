use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InlineLayoutPlan {
    block_start_row: u16,
    draw_count: usize,
    skip: usize,
}

fn plan_inline_layout(
    height: usize,
    frame_len: usize,
    prev_block_start_row: u16,
) -> InlineLayoutPlan {
    if height == 0 {
        return InlineLayoutPlan {
            block_start_row: 0,
            draw_count: 0,
            skip: 0,
        };
    }

    let max_row = height.saturating_sub(1) as u16;
    let mut block_start = prev_block_start_row.min(max_row) as usize;
    let desired_visible = frame_len.min(height);
    let available = height.saturating_sub(block_start);

    if desired_visible > available {
        let need = desired_visible.saturating_sub(available);
        let shift_up = need.min(block_start);
        block_start = block_start.saturating_sub(shift_up);
    }

    let available_after_shift = height.saturating_sub(block_start);
    let draw_count = frame_len.min(available_after_shift);
    let skip = frame_len.saturating_sub(draw_count);
    let block_start_row = block_start.min(u16::MAX as usize) as u16;

    InlineLayoutPlan {
        block_start_row,
        draw_count,
        skip,
    }
}

fn next_row(row: u16) -> Option<u16> {
    row.checked_add(1)
}

impl Terminal {
    fn draw_dirty_rows(
        &mut self,
        frame: &RenderFrame,
        width: u16,
        source_offset: usize,
        target_row_offset: u16,
        draw_count: usize,
        dirty_rows: &DirtyRows,
    ) -> io::Result<()> {
        for range in dirty_rows.ranges() {
            let mut row = range.start;
            loop {
                let target_row = target_row_offset.saturating_add(row);
                queue!(
                    self.stdout,
                    MoveTo(0, target_row),
                    Clear(ClearType::CurrentLine)
                )?;
                if (row as usize) < draw_count
                    && let Some(line) = frame.lines.get(source_offset + row as usize)
                {
                    self.write_span_line(line, width)?;
                }
                if row == range.end_inclusive {
                    break;
                }
                if let Some(next) = next_row(row) {
                    row = next;
                } else {
                    break;
                }
            }
        }
        Ok(())
    }

    fn queue_cursor_state(
        &mut self,
        position: Option<(u16, u16)>,
        cursor_visible: bool,
        hidden_anchor: Option<(u16, u16)>,
    ) -> io::Result<()> {
        match position {
            Some((col, row)) => {
                queue!(self.stdout, MoveTo(col, row))?;
                if cursor_visible {
                    queue!(self.stdout, Show)?;
                } else {
                    queue!(self.stdout, Hide)?;
                }
            }
            None => {
                if let Some((col, row)) = hidden_anchor {
                    queue!(self.stdout, MoveTo(col, row))?;
                }
                queue!(self.stdout, Hide)?;
            }
        }
        Ok(())
    }

    pub fn render_frame(&mut self, frame: &RenderFrame) -> io::Result<()> {
        self.refresh_size()?;
        self.state.cursor = frame.cursor;
        self.state.cursor_visible = frame.cursor_visible;
        match self.mode {
            RenderMode::AltScreen => self.render_altscreen(frame),
            RenderMode::Inline => self.render_inline(frame),
        }
    }

    fn render_altscreen(&mut self, frame: &RenderFrame) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        if height == 0 || width == 0 {
            return Ok(());
        }

        let frame_len = frame.lines.len();
        let frame_signature = quick_frame_signature(frame.lines.as_slice());
        let max_offset = frame_len.saturating_sub(height);
        let scroll_offset = {
            let alt = self
                .alt_screen
                .as_mut()
                .expect("alt_screen must be Some in AltScreen mode");

            if !alt.manually_scrolled {
                alt.scroll_offset = match frame.cursor {
                    Some(cur) => (cur.row as usize).saturating_sub(height.saturating_sub(1)),
                    None => max_offset,
                };
            }

            alt.scroll_offset = alt.scroll_offset.min(max_offset);
            alt.scroll_offset
        };

        let (dirty_rows, skip_noop) = {
            let alt = self
                .alt_screen
                .as_ref()
                .expect("alt_screen must be Some in AltScreen mode");
            let size_changed = alt.last_rendered_size != self.state.size;
            let offset_same = alt.last_rendered_scroll_offset == scroll_offset;
            let cursor_same = alt.last_rendered_cursor == frame.cursor
                && alt.last_rendered_cursor_visible == frame.cursor_visible
                && alt.last_rendered_size == self.state.size;
            let frame_same =
                alt.last_frame_signature == frame_signature && alt.last_frame == frame.lines;
            if frame_same && cursor_same && offset_same && !size_changed {
                (DirtyRows::default(), true)
            } else {
                let dirty = compute_dirty_rows(
                    if alt.has_rendered_once {
                        Some(alt.last_frame.as_slice())
                    } else {
                        None
                    },
                    alt.last_rendered_scroll_offset,
                    frame.lines.as_slice(),
                    scroll_offset,
                    height,
                    size_changed,
                );
                let dirty_is_empty = dirty.is_empty();
                (dirty, cursor_same && dirty_is_empty)
            }
        };
        if skip_noop {
            if let Some(alt) = self.alt_screen.as_mut() {
                alt.last_frame.clone_from(&frame.lines);
                alt.last_frame_signature = frame_signature;
                alt.last_rendered_cursor = frame.cursor;
                alt.last_rendered_cursor_visible = frame.cursor_visible;
                alt.last_rendered_size = self.state.size;
                alt.last_rendered_scroll_offset = scroll_offset;
                alt.has_rendered_once = true;
            }
            return Ok(());
        }

        queue!(self.stdout, BeginSynchronizedUpdate, Hide)?;
        self.draw_dirty_rows(frame, width, scroll_offset, 0, height, &dirty_rows)?;

        let cursor_position = frame.cursor.and_then(|cur| {
            let frame_row = cur.row as usize;
            if frame_row < scroll_offset {
                return None;
            }
            let screen_row = frame_row - scroll_offset;
            if screen_row >= height {
                return None;
            }
            Some((cur.col.min(width.saturating_sub(1)), screen_row as u16))
        });
        self.queue_cursor_state(cursor_position, frame.cursor_visible, None)?;
        queue!(self.stdout, EndSynchronizedUpdate)?;

        if let Some(alt) = self.alt_screen.as_mut() {
            alt.last_frame.clone_from(&frame.lines);
            alt.last_frame_signature = frame_signature;
            alt.last_rendered_cursor = frame.cursor;
            alt.last_rendered_cursor_visible = frame.cursor_visible;
            alt.last_rendered_size = self.state.size;
            alt.last_rendered_scroll_offset = scroll_offset;
            alt.has_rendered_once = true;
        }

        self.stdout.flush()
    }

    fn render_inline(&mut self, frame: &RenderFrame) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;
        let frame_signature = quick_frame_signature(frame.lines.as_slice());

        if height == 0 || width == 0 {
            return Ok(());
        }

        self.reanchor_inline_after_resize_if_needed();

        let (
            prev_anchor_row,
            prev_rendered_block_start_row,
            prev_drawn,
            _prev_cursor_row,
            skip_noop,
        ) = self
            .inline_state
            .as_ref()
            .map(|inline| {
                let same_frame = inline.last_frame_signature == frame_signature
                    && inline.last_frame == frame.lines;
                let same_cursor = inline.last_rendered_cursor == frame.cursor;
                let same_cursor_visibility =
                    inline.last_rendered_cursor_visible == frame.cursor_visible;
                let same_size = inline.last_rendered_size == self.state.size;
                let should_skip = inline.has_rendered_once
                    && !inline.reanchor_after_resize
                    && same_frame
                    && same_cursor
                    && same_cursor_visibility
                    && same_size;
                (
                    inline.block_start_row,
                    inline.last_rendered_block_start_row,
                    inline.last_drawn_count,
                    inline.last_cursor_row,
                    should_skip,
                )
            })
            .unwrap_or((0, 0, 0, 0, false));
        if skip_noop {
            return Ok(());
        }

        let mut next_anchor_row = prev_anchor_row;
        let mut prev_rendered_row = prev_rendered_block_start_row;
        let frame_len = frame.lines.len();

        let plan = plan_inline_layout(height, frame_len, prev_rendered_row);
        let block_start_row = plan.block_start_row;
        let block_start = block_start_row as usize;
        let draw_count = plan.draw_count;
        let skip = plan.skip;
        let scroll_up_lines = prev_rendered_row.saturating_sub(block_start_row);
        if scroll_up_lines > 0 {
            next_anchor_row = next_anchor_row.saturating_sub(scroll_up_lines);
            prev_rendered_row = prev_rendered_row.saturating_sub(scroll_up_lines);
        }
        let clear_start_row = prev_rendered_row.min(block_start_row);
        let can_diff_render = scroll_up_lines == 0 && block_start_row == prev_rendered_row;

        let (dirty_rows, size_changed) = if can_diff_render {
            let inline = self
                .inline_state
                .as_ref()
                .expect("inline_state must be Some");
            let old_skip = inline.last_frame.len().saturating_sub(prev_drawn);
            let row_count = draw_count.max(prev_drawn);
            let size_changed = inline.last_rendered_size != self.state.size;
            (
                compute_dirty_rows(
                    if inline.has_rendered_once {
                        Some(inline.last_frame.as_slice())
                    } else {
                        None
                    },
                    old_skip,
                    frame.lines.as_slice(),
                    skip,
                    row_count,
                    size_changed,
                ),
                size_changed,
            )
        } else {
            (DirtyRows::default(), false)
        };

        queue!(self.stdout, BeginSynchronizedUpdate, Hide)?;
        if !can_diff_render || size_changed {
            if scroll_up_lines > 0 {
                queue!(
                    self.stdout,
                    MoveTo(0, self.state.size.height.saturating_sub(1)),
                    ScrollUp(scroll_up_lines)
                )?;
            }
            queue!(
                self.stdout,
                MoveTo(0, clear_start_row),
                Clear(ClearType::FromCursorDown)
            )?;
            for visible_row in 0..draw_count {
                let target_row = block_start.saturating_add(visible_row) as u16;
                queue!(self.stdout, MoveTo(0, target_row))?;
                if let Some(line) = frame.lines.get(skip + visible_row) {
                    self.write_span_line(line, width)?;
                }
            }
        } else {
            self.draw_dirty_rows(frame, width, skip, block_start_row, draw_count, &dirty_rows)?;
        }

        let mut next_last_cursor_row = 0u16;
        let mut next_last_cursor_col = 0u16;
        let cursor_position = if let Some(cursor) = frame.cursor {
            let cursor_row = cursor.row as usize;
            if cursor_row >= skip {
                let visible_row = cursor_row - skip;
                let target_row = block_start + visible_row;
                let col = cursor.col.min(width.saturating_sub(1));
                next_last_cursor_row = visible_row.min(u16::MAX as usize) as u16;
                next_last_cursor_col = col;
                Some((col, target_row as u16))
            } else {
                None
            }
        } else {
            None
        };

        self.queue_cursor_state(
            cursor_position,
            frame.cursor_visible,
            Some((0, block_start_row)),
        )?;

        if let Some(inline) = self.inline_state.as_mut() {
            inline.last_frame.clone_from(&frame.lines);
            inline.last_frame_signature = frame_signature;
            inline.last_drawn_count = draw_count;
            inline.last_cursor_row = next_last_cursor_row;
            inline.last_cursor_col = next_last_cursor_col;
            inline.block_start_row = next_anchor_row;
            inline.last_rendered_block_start_row = block_start_row;
            inline.last_rendered_cursor = frame.cursor;
            inline.last_rendered_cursor_visible = frame.cursor_visible;
            inline.last_rendered_size = self.state.size;
            inline.has_rendered_once = true;
        }

        queue!(self.stdout, EndSynchronizedUpdate)?;

        self.stdout.flush()
    }
}
