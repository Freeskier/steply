use crate::state::step::StepStatus;
use crate::terminal::TerminalSize;
use crate::ui::render_view::RenderView;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};

use super::focus_policy::apply_focus_cursor_state;
use super::step_content::{
    active_focus_id, append_step_hints_lines, apply_step_decoration, render_step_content,
    render_step_hints, resolve_step_focus_cursor, step_frame_footer,
};
use super::step_decoration::decoration_gutter_width;
use super::{
    FocusApplyMode, RenderFrame, RendererConfig, StepRenderRange, StepVisualStatus,
    register_block_selection_ranges, status_allows_interaction,
};

pub(super) fn build_base_frame(
    view: &RenderView,
    terminal_size: TerminalSize,
    config: RendererConfig,
    running_marker: char,
) -> RenderFrame {
    let mut frame = RenderFrame::default();
    let current_idx = view.current_step_index;
    let steps = &view.steps;
    if steps.is_empty() {
        frame.lines.push(vec![Span::styled(
            "No steps configured.",
            Style::new().color(Color::Red).bold(),
        )]);
        return frame;
    }
    let blocking_overlay = view.has_blocking_overlay;
    let compose_width = if config.chrome_enabled {
        terminal_size
            .width
            .saturating_sub(decoration_gutter_width().min(u16::MAX as usize) as u16)
            .max(1)
    } else {
        terminal_size.width
    };
    let node_terminal_size = TerminalSize {
        width: compose_width,
        height: terminal_size.height,
    };

    let last_visible_idx = view
        .step_statuses
        .iter()
        .enumerate()
        .rev()
        .find(|(_, s)| !matches!(s, StepStatus::Pending))
        .map(|(i, _)| i)
        .unwrap_or(current_idx);
    let render_up_to = last_visible_idx.max(current_idx);

    for (idx, step) in steps
        .iter()
        .enumerate()
        .take(render_up_to.saturating_add(1))
    {
        let status = StepVisualStatus::from(view.step_statuses[idx]);
        let focused_id = active_focus_id(status, blocking_overlay, view.focused_id);
        let mut content = render_step_content(
            view,
            step,
            status,
            focused_id,
            node_terminal_size,
            compose_width,
        );
        let hints = render_step_hints(status, view, step.nodes.as_slice());
        let footer = step_frame_footer(status, view, hints.has_hints);
        apply_step_decoration(
            &mut content,
            compose_width,
            idx,
            render_up_to,
            status,
            config,
            footer,
            running_marker,
        );

        let start_row = frame.lines.len() as u16;
        let block_len = content.lines.len().min(u16::MAX as usize) as u16;
        if status_allows_interaction(status) {
            frame.active_step_range = Some(StepRenderRange {
                start: start_row,
                end_exclusive: start_row.saturating_add(block_len),
            });
        }
        let selection_col_start = if config.chrome_enabled {
            decoration_gutter_width().min(u16::MAX as usize) as u16
        } else {
            0
        };
        register_block_selection_ranges(
            &mut content.hit_map,
            content.lines.as_slice(),
            selection_col_start,
        );
        let focus_cursor = resolve_step_focus_cursor(start_row, &content, focused_id, status);
        content.hit_map.shift_rows(start_row);
        frame.hit_map.extend(content.hit_map);
        frame.lines.extend(content.lines);
        frame.sticky.extend(content.sticky);
        apply_focus_cursor_state(&mut frame, focus_cursor, FocusApplyMode::PreserveExisting);

        if status == StepVisualStatus::Done && !config.chrome_enabled {
            frame.lines.push(vec![Span::new("")]);
        }

        let hint_line_count = hints.panel_lines.len().min(u16::MAX as usize) as u16;
        append_step_hints_lines(
            &mut frame.lines,
            hints.panel_lines,
            config.chrome_enabled,
            idx < render_up_to,
        );
        if status_allows_interaction(status)
            && let Some(range) = frame.active_step_range.as_mut()
        {
            range.end_exclusive = range.end_exclusive.saturating_add(hint_line_count);
        }
    }
    frame
}
