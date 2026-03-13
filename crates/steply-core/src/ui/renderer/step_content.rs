use crate::state::step::Step;
use crate::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::render_view::RenderView;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::node::Node;

use super::focus_policy::{
    focused_cursor_in_hit_map, layout_marker_from_focus, resolve_focus_anchor,
};
use super::hints_panel::{collect_hints, render_hints_panel_lines};
use super::render_context::{render_context_for_nodes, tint_block};
use super::step_decoration::{
    StepFrameFooter, append_step_frame_footer_plain, apply_step_frame, decoration_gutter_width,
    hint_line_prefix,
};
use super::{
    DrawNodesOptions, DrawNodesState, FocusCursorState, RendererConfig, StepContentRender,
    StepHintsRender, StepVisualStatus, draw_nodes, status_allows_interaction,
};

pub(super) fn active_focus_id(
    status: StepVisualStatus,
    blocking_overlay: bool,
    focused_id: Option<&str>,
) -> Option<&str> {
    if status_allows_interaction(status) && !blocking_overlay {
        focused_id
    } else {
        None
    }
}

pub(super) fn render_step_content(
    view: &RenderView<'_>,
    step: &Step,
    status: StepVisualStatus,
    focused_id: Option<&str>,
    node_terminal_size: TerminalSize,
    compose_width: u16,
) -> StepContentRender {
    let mut content = StepContentRender::default();
    let mut row_offset: u16 = 0;

    if !step.prompt.trim().is_empty() {
        content.lines.push(vec![Span::styled(
            format!("{} [{}]", step.prompt, step.id),
            step_title_style(status),
        )]);
        row_offset = row_offset.saturating_add(1);
    }

    if let Some(description) = step.description.as_deref() {
        content.lines.push(vec![Span::styled(
            format!("Description: {}", description),
            step_description_style(status),
        )]);
        row_offset = row_offset.saturating_add(1);
    }

    let is_active_interaction_pass =
        status_allows_interaction(status) && !view.has_blocking_overlay;
    let ctx = render_context_for_nodes(
        view.validation,
        view.completion.as_ref(),
        node_terminal_size,
        status,
        step.nodes.as_slice(),
        focused_id,
    );
    let mut hit_row_offset = Layout::compose(&content.lines, compose_width).len() as u16;
    let mut draw_state = DrawNodesState {
        lines: &mut content.lines,
        sticky: Some(&mut content.sticky),
        cursor: &mut content.cursor,
        focus_anchor: &mut content.focus_anchor,
        cursor_visible: &mut content.cursor_visible,
        row_offset: &mut row_offset,
        hit_map: Some(&mut content.hit_map),
        hit_row_offset: Some(&mut hit_row_offset),
        hit_col_start: 0,
        compose_width,
    };
    draw_nodes(
        step.nodes.as_slice(),
        &ctx,
        &mut draw_state,
        DrawNodesOptions {
            track_cursor: is_active_interaction_pass,
            strikethrough_inputs: status == StepVisualStatus::Cancelled,
            collect_sticky: is_active_interaction_pass,
        },
    );

    remap_block_layout_marker(
        &mut content.lines,
        compose_width,
        &mut content.cursor,
        &mut content.focus_anchor,
    );

    if let Some(tint) = step_content_tint(status) {
        tint_block(&mut content.lines, tint);
    }

    content
}

pub(super) fn apply_step_decoration<'a>(
    content: &mut StepContentRender,
    compose_width: u16,
    idx: usize,
    render_up_to: usize,
    status: StepVisualStatus,
    config: RendererConfig,
    footer: Option<StepFrameFooter<'a>>,
    running_marker: char,
) {
    if config.chrome_enabled {
        let include_top = idx == 0;
        apply_step_frame(
            &mut content.lines,
            &mut content.cursor,
            compose_width,
            idx < render_up_to,
            status,
            include_top,
            footer,
            running_marker,
        );
        if include_top {
            content.hit_map.shift_rows(1);
            if let Some(anchor) = content.focus_anchor.as_mut() {
                *anchor = anchor.saturating_add(1);
            }
        }
        content
            .hit_map
            .shift_cols(decoration_gutter_width().min(u16::MAX as usize) as u16);
    } else {
        append_step_frame_footer_plain(&mut content.lines, compose_width, footer);
    }
}

pub(super) fn resolve_step_focus_cursor(
    start_row: u16,
    content: &StepContentRender,
    focused_id: Option<&str>,
    status: StepVisualStatus,
) -> FocusCursorState {
    let cursor =
        focused_cursor_in_hit_map(focused_id, content.cursor, &content.hit_map).map(|cursor| {
            CursorPos {
                row: cursor.row.saturating_add(start_row),
                col: cursor.col,
            }
        });

    let focus_anchor = resolve_focus_anchor(
        focused_id,
        &content.hit_map,
        status_allows_interaction(status),
        content.focus_anchor,
    )
    .map(|(row, col)| CursorPos {
        row: row.saturating_add(start_row),
        col,
    });

    FocusCursorState {
        cursor,
        focus_anchor,
        cursor_visible: content.cursor_visible,
    }
}

pub(super) fn step_frame_footer<'a>(
    status: StepVisualStatus,
    view: &'a RenderView<'a>,
    has_hints: bool,
) -> Option<StepFrameFooter<'a>> {
    if status == StepVisualStatus::Cancelled {
        return Some(StepFrameFooter::Error {
            message: "Application terminated.",
            description: None,
            show_help_toggle: false,
        });
    }

    if !status_allows_interaction(status) {
        return None;
    }

    if let Some(choice) = view.exit_confirm {
        return Some(StepFrameFooter::ExitConfirm { choice });
    }

    if let Some(msg) = view.back_confirm {
        return Some(StepFrameFooter::Warning {
            message: msg,
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    if let Some(msg) = view.step_errors.first() {
        return Some(StepFrameFooter::Error {
            message: msg.as_str(),
            description: None,
            show_help_toggle: has_hints,
        });
    }

    if let Some(msg) = view.step_warnings.first() {
        return Some(StepFrameFooter::Warning {
            message: msg.as_str(),
            description: Some("[Enter] confirm  •  [Esc] cancel"),
            show_help_toggle: false,
        });
    }

    has_hints.then_some(StepFrameFooter::HelpToggle)
}

pub(super) fn render_step_hints(
    status: StepVisualStatus,
    view: &RenderView<'_>,
    nodes: &[Node],
) -> StepHintsRender {
    if !status_allows_interaction(status) {
        return StepHintsRender::default();
    }

    let hints = collect_hints(nodes, view.focused_id);
    let has_hints = !hints.is_empty();
    let has_active_warning_or_error = view.exit_confirm.is_some()
        || view.back_confirm.is_some()
        || !view.step_errors.is_empty()
        || !view.step_warnings.is_empty();
    let panel_lines = if view.hints_visible && !has_active_warning_or_error {
        render_hints_panel_lines(hints)
    } else {
        Vec::new()
    };

    StepHintsRender {
        has_hints,
        panel_lines,
    }
}

pub(super) fn append_step_hints_lines(
    frame_lines: &mut Vec<SpanLine>,
    hints_panel_lines: Vec<SpanLine>,
    chrome_enabled: bool,
    connect_to_next: bool,
) {
    if hints_panel_lines.is_empty() {
        return;
    }
    if chrome_enabled {
        let hint_prefix = hint_line_prefix(connect_to_next);
        for line in hints_panel_lines {
            let mut prefixed = vec![hint_prefix.clone()];
            prefixed.extend(line);
            frame_lines.push(prefixed);
        }
    } else {
        frame_lines.extend(hints_panel_lines);
    }
}

fn remap_block_layout_marker(
    block_lines: &mut Vec<SpanLine>,
    compose_width: u16,
    block_cursor: &mut Option<CursorPos>,
    block_focus_anchor: &mut Option<u16>,
) {
    let layout_marker = layout_marker_from_focus(*block_cursor, None, *block_focus_anchor);
    let (composed_lines, mapped_marker) =
        Layout::compose_with_cursor(block_lines.as_slice(), compose_width, layout_marker);
    *block_lines = composed_lines;
    if block_cursor.is_some() {
        *block_cursor = mapped_marker.map(|(row, col)| CursorPos {
            row: row.min(u16::MAX as usize) as u16,
            col: col.min(u16::MAX as usize) as u16,
        });
    } else {
        *block_focus_anchor = mapped_marker.map(|(row, _)| row.min(u16::MAX as usize) as u16);
    }
}

fn step_title_style(status: StepVisualStatus) -> Style {
    match status {
        StepVisualStatus::Active | StepVisualStatus::Running => Style::new().color(Color::Cyan),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    }
}

fn step_description_style(status: StepVisualStatus) -> Style {
    match status {
        StepVisualStatus::Active | StepVisualStatus::Running => Style::new().color(Color::Yellow),
        StepVisualStatus::Done | StepVisualStatus::Pending => Style::new().color(Color::DarkGrey),
        StepVisualStatus::Cancelled => Style::new().color(Color::Red),
    }
}

fn step_content_tint(status: StepVisualStatus) -> Option<Color> {
    match status {
        StepVisualStatus::Cancelled | StepVisualStatus::Done | StepVisualStatus::Pending => {
            Some(Color::DarkGrey)
        }
        StepVisualStatus::Active | StepVisualStatus::Running => None,
    }
}
