use super::InlineState;
use crate::ui::span::SpanLine;
use crate::ui::text::char_display_width;

pub(super) fn compute_dirty_rows(
    prev_lines: Option<&[SpanLine]>,
    prev_offset: usize,
    next_lines: &[SpanLine],
    next_offset: usize,
    row_count: usize,
    force_all: bool,
) -> Vec<u16> {
    if force_all || prev_lines.is_none() {
        return (0..row_count.min(u16::MAX as usize))
            .map(|row| row as u16)
            .collect();
    }

    let prev_lines = prev_lines.expect("checked above");
    let mut dirty = Vec::new();
    for row in 0..row_count {
        let prev_line = prev_lines.get(prev_offset + row);
        let next_line = next_lines.get(next_offset + row);
        if prev_line != next_line {
            if row > u16::MAX as usize {
                break;
            }
            dirty.push(row as u16);
        }
    }
    dirty
}

pub(super) fn estimate_self_reflow_cursor_delta(inline: &InlineState, new_width: u16) -> i32 {
    if !inline.has_rendered_once || inline.last_drawn_count == 0 {
        return 0;
    }
    let old_width = inline.last_rendered_size.width;
    if old_width == 0 || new_width == 0 {
        return 0;
    }

    let old_skip = inline
        .last_frame
        .len()
        .saturating_sub(inline.last_drawn_count);
    let visible_lines = &inline.last_frame[old_skip..];
    if visible_lines.is_empty() {
        return 0;
    }

    let cursor = match inline.last_rendered_cursor {
        Some(cursor) => cursor,
        None => return 0,
    };
    let cursor_abs_row = cursor.row as usize;
    if cursor_abs_row < old_skip {
        return 0;
    }
    let cursor_visible_row = inline
        .last_cursor_row
        .min(visible_lines.len().saturating_sub(1) as u16) as usize;

    let new_width_usize = new_width as usize;
    let mut new_row = 0usize;
    for line in visible_lines.iter().take(cursor_visible_row) {
        let width = rendered_line_width(line, old_width);
        new_row = new_row.saturating_add(wrapped_rows(width, new_width_usize));
    }

    if visible_lines.get(cursor_visible_row).is_none() {
        return 0;
    }

    let prefix = inline.last_cursor_col.min(old_width.saturating_sub(1)) as usize;
    new_row = new_row.saturating_add(prefix / new_width_usize);

    new_row as i32 - cursor_visible_row as i32
}

fn rendered_line_width(line: &SpanLine, old_width: u16) -> usize {
    let render_width = if old_width > 1 {
        (old_width - 1) as usize
    } else {
        old_width as usize
    };
    if render_width == 0 {
        return 0;
    }

    let mut used = 0usize;
    for span in line {
        if used >= render_width {
            break;
        }
        let available = render_width.saturating_sub(used);
        let mut span_used = 0usize;
        for ch in span.text.chars().filter(|ch| !matches!(ch, '\n' | '\r')) {
            let ch_width = char_display_width(ch);
            if span_used.saturating_add(ch_width) > available {
                break;
            }
            span_used = span_used.saturating_add(ch_width);
        }
        used = used.saturating_add(span_used);
    }
    used
}

fn wrapped_rows(line_width: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if line_width == 0 {
        return 1;
    }
    (line_width.saturating_sub(1) / width).saturating_add(1)
}
