use crate::terminal::TerminalSize;
use crate::widgets::traits::{OverlayPlacement, OverlayRenderMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayGeometry {
    Floating(FloatingOverlayGeometry),
    Inline(InlineOverlayGeometry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FloatingOverlayGeometry {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub height: u16,
    pub content_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InlineOverlayGeometry {
    pub insert_row: usize,
    pub gutter_width: usize,
    pub left_padding_cols: usize,
    pub content_width: u16,
}

impl InlineOverlayGeometry {
    pub fn content_col_offset(self) -> usize {
        self.gutter_width.saturating_add(self.left_padding_cols)
    }
}

pub(super) fn resolve_overlay_geometry(
    placement: OverlayPlacement,
    terminal_size: TerminalSize,
    frame_line_count: usize,
    decoration_gutter_width: usize,
) -> OverlayGeometry {
    match placement.render_mode {
        OverlayRenderMode::Floating => OverlayGeometry::Floating(FloatingOverlayGeometry {
            row: placement.row,
            col: placement.col,
            width: placement.width,
            height: placement.height,
            content_width: placement.width.saturating_sub(2).max(1),
        }),
        OverlayRenderMode::Inline => {
            let left_padding_cols =
                (placement.col as usize).saturating_sub(decoration_gutter_width);
            let content_width = terminal_size
                .width
                .saturating_sub((decoration_gutter_width.saturating_add(left_padding_cols)) as u16)
                .max(1);

            OverlayGeometry::Inline(InlineOverlayGeometry {
                insert_row: (placement.row as usize).min(frame_line_count),
                gutter_width: decoration_gutter_width,
                left_padding_cols,
                content_width,
            })
        }
    }
}
