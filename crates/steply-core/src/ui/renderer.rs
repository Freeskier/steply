use crate::state::step::StepStatus;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::hit_test::FrameHitMap;
use crate::ui::render_view::RenderView;
use crate::ui::span::SpanLine;
use crate::ui::spinner::{Spinner, SpinnerStyle};
use crate::widgets::traits::StickyBlock;

mod content_render;
mod focus_policy;
mod frame_build;
mod hints_panel;
mod overlay;
mod overlay_geometry;
mod render_context;
mod step_content;
mod step_decoration;

pub(super) use content_render::{draw_nodes, register_block_selection_ranges};
pub(super) use focus_policy::{
    apply_focus_cursor_state, focused_cursor_in_hit_map, layout_marker_from_focus,
    resolve_focus_anchor,
};
use frame_build::build_base_frame;
use overlay::apply_overlay;

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub sticky: Vec<StickyBlock>,
    pub cursor: Option<CursorPos>,
    pub focus_anchor_row: Option<u16>,
    pub focus_anchor_col: Option<u16>,
    pub active_step_range: Option<StepRenderRange>,
    pub cursor_visible: bool,
    pub hit_map: FrameHitMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepRenderRange {
    pub start: u16,
    pub end_exclusive: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererConfig {
    pub chrome_enabled: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            chrome_enabled: true,
        }
    }
}

pub struct Renderer {
    config: RendererConfig,
    running_spinner: Spinner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepVisualStatus {
    Pending,
    Done,
    Active,
    Running,
    Cancelled,
}

pub(super) struct DrawNodesState<'a> {
    pub lines: &'a mut Vec<SpanLine>,
    pub sticky: Option<&'a mut Vec<StickyBlock>>,
    pub cursor: &'a mut Option<CursorPos>,
    pub focus_anchor: &'a mut Option<u16>,
    pub cursor_visible: &'a mut bool,
    pub row_offset: &'a mut u16,
    pub hit_map: Option<&'a mut FrameHitMap>,
    pub hit_row_offset: Option<&'a mut u16>,
    pub hit_col_start: u16,
    pub compose_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DrawNodesOptions {
    pub track_cursor: bool,
    pub strikethrough_inputs: bool,
    pub collect_sticky: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct StepContentRender {
    pub lines: Vec<SpanLine>,
    pub sticky: Vec<StickyBlock>,
    pub cursor: Option<CursorPos>,
    pub focus_anchor: Option<u16>,
    pub cursor_visible: bool,
    pub hit_map: FrameHitMap,
}

#[derive(Debug, Clone, Default)]
pub(super) struct StepHintsRender {
    pub has_hints: bool,
    pub panel_lines: Vec<SpanLine>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FocusCursorState {
    pub cursor: Option<CursorPos>,
    pub focus_anchor: Option<CursorPos>,
    pub cursor_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusApplyMode {
    PreserveExisting,
    OverrideExisting,
}

impl From<StepStatus> for StepVisualStatus {
    fn from(value: StepStatus) -> Self {
        match value {
            StepStatus::Pending => Self::Pending,
            StepStatus::Active => Self::Active,
            StepStatus::Running => Self::Running,
            StepStatus::Done => Self::Done,
            StepStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl Renderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            running_spinner: Spinner::new(SpinnerStyle::Arc),
        }
    }

    pub fn render(&mut self, view: &RenderView, terminal_size: TerminalSize) -> RenderFrame {
        let layout_terminal_size = effective_layout_terminal_size(terminal_size);
        let running_marker = self.running_spinner.glyph();
        self.running_spinner.tick();
        let mut frame = self.render_steps_pass(view, layout_terminal_size, running_marker);
        self.apply_overlay_pass(view, layout_terminal_size, &mut frame);
        self.finalize_cursor_pass(layout_terminal_size, &mut frame);
        frame
    }

    fn render_steps_pass(
        &self,
        view: &RenderView,
        terminal_size: TerminalSize,
        running_marker: char,
    ) -> RenderFrame {
        build_base_frame(view, terminal_size, self.config, running_marker)
    }

    fn apply_overlay_pass(
        &self,
        view: &RenderView,
        terminal_size: TerminalSize,
        frame: &mut RenderFrame,
    ) {
        for overlay_view in &view.overlays {
            let focused_id = if overlay_view.is_topmost {
                view.focused_id
            } else {
                None
            };
            apply_overlay(
                view.validation,
                view.completion.as_ref(),
                terminal_size,
                overlay_view.nodes,
                overlay_view.placement,
                focused_id,
                frame,
            );
        }
    }

    fn finalize_cursor_pass(&self, terminal_size: TerminalSize, frame: &mut RenderFrame) {
        if let Some(cursor) = frame.cursor.as_mut() {
            if terminal_size.width > 0 {
                cursor.col = cursor.col.min(terminal_size.width.saturating_sub(1));
            }
            let max_row = frame.lines.len().saturating_sub(1) as u16;
            if cursor.row > max_row {
                cursor.row = max_row;
            }
        }
        if let Some(anchor_row) = frame.focus_anchor_row.as_mut() {
            let max_row = frame.lines.len().saturating_sub(1) as u16;
            if *anchor_row > max_row {
                *anchor_row = max_row;
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RendererConfig::default())
    }
}

fn effective_layout_terminal_size(size: TerminalSize) -> TerminalSize {
    TerminalSize {
        width: if size.width > 1 {
            size.width.saturating_sub(1)
        } else {
            size.width
        },
        height: size.height,
    }
}
