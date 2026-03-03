use super::{FocusApplyMode, FocusCursorState, RenderFrame};
use crate::terminal::CursorPos;
use crate::ui::hit_test::FrameHitMap;

pub(crate) fn focused_cursor_in_hit_map(
    focused_id: Option<&str>,
    cursor: Option<CursorPos>,
    hit_map: &FrameHitMap,
) -> Option<CursorPos> {
    match (focused_id, cursor) {
        (Some(id), Some(cursor)) if hit_map.has_node_row(id, cursor.row) => Some(cursor),
        (Some(_), Some(_)) => None,
        (_, cursor) => cursor,
    }
}

pub(crate) fn resolve_focus_anchor(
    focused_id: Option<&str>,
    hit_map: &FrameHitMap,
    fallback_to_first_region: bool,
    fallback_row_anchor: Option<u16>,
) -> Option<(u16, u16)> {
    focused_id
        .and_then(|id| hit_map.first_region_for_node(id))
        .or_else(|| {
            fallback_to_first_region
                .then(|| hit_map.first_region())
                .flatten()
        })
        .or_else(|| fallback_row_anchor.map(|row| (row, 0)))
}

pub(crate) fn layout_marker_from_focus(
    cursor: Option<CursorPos>,
    focus_anchor: Option<(u16, u16)>,
    fallback_row_anchor: Option<u16>,
) -> Option<(usize, usize)> {
    cursor
        .map(|local| (local.row as usize, local.col as usize))
        .or_else(|| focus_anchor.map(|(row, col)| (row as usize, col as usize)))
        .or_else(|| fallback_row_anchor.map(|row| (row as usize, 0usize)))
}

pub(crate) fn apply_focus_cursor_state(
    frame: &mut RenderFrame,
    state: FocusCursorState,
    mode: FocusApplyMode,
) {
    let override_existing = mode == FocusApplyMode::OverrideExisting;

    if let Some(cursor) = state.cursor
        && (override_existing || frame.cursor.is_none())
    {
        frame.cursor = Some(cursor);
        frame.focus_anchor_row = Some(cursor.row);
        frame.focus_anchor_col = Some(cursor.col);
        frame.cursor_visible = state.cursor_visible;
        return;
    }

    if let Some(anchor) = state.focus_anchor
        && (override_existing || frame.focus_anchor_row.is_none())
    {
        frame.focus_anchor_row = Some(anchor.row);
        frame.focus_anchor_col = Some(anchor.col);
    }
}
