use crate::terminal::CursorPos;

pub fn anchored_cursor(row: usize, col: u16) -> CursorPos {
    CursorPos {
        col,
        row: row.min(u16::MAX as usize) as u16,
    }
}

pub fn first_col_cursor(row: usize) -> CursorPos {
    anchored_cursor(row, 0)
}

pub fn visible_row_cursor(
    active_index: usize,
    start: usize,
    end: usize,
    prefix_rows: usize,
    col: u16,
) -> Option<CursorPos> {
    if active_index < start || active_index >= end {
        return None;
    }
    let row = prefix_rows.saturating_add(active_index.saturating_sub(start));
    Some(anchored_cursor(row, col))
}

pub fn visible_when_text_cursor(has_text_cursor: bool) -> bool {
    has_text_cursor
}
