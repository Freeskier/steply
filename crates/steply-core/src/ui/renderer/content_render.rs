use super::{DrawNodesOptions, DrawNodesState};
use crate::terminal::CursorPos;
use crate::ui::hit_test::{FrameHitMap, HitLocal};
use crate::ui::layout::Layout;
use crate::ui::span::{Span, SpanLine, WrapMode};
use crate::ui::style::{Color, Strike, Style};
use crate::ui::text::text_display_width;
use crate::widgets::node::Node;
use crate::widgets::traits::{DrawOutput, PointerRowMap, RenderContext};
use std::collections::HashMap;

pub(crate) fn draw_nodes(
    nodes: &[Node],
    ctx: &RenderContext,
    state: &mut DrawNodesState<'_>,
    options: DrawNodesOptions,
) {
    for node in nodes {
        let (label_prefix, label_offset) = input_label_prefix(node, ctx.focused_id.as_deref());
        let draw_ctx = if label_offset > 0 {
            ctx.with_terminal_width(ctx.terminal_size.width.saturating_sub(label_offset))
        } else {
            ctx.with_focus(ctx.focused_id.clone())
        };
        let mut out = node.draw(&draw_ctx);

        apply_input_validation_overlay(node, ctx, &mut out);
        if options.strikethrough_inputs && matches!(node, Node::Input(_)) {
            for line in &mut out.lines {
                let has_content = line
                    .iter()
                    .any(|s| !s.text.trim().is_empty() && !matches!(s.style.strike, Strike::Off));
                if has_content {
                    for span in line.iter_mut() {
                        if !matches!(span.style.strike, Strike::Off) {
                            span.style.strike = Strike::On;
                        }
                    }
                }
            }
        }

        if let Some(prefix) = label_prefix {
            if let Some(first) = out.lines.first_mut() {
                let mut new_first = prefix;
                new_first.append(first);
                *first = new_first;
            } else {
                out.lines.insert(0, prefix);
            }
        }
        enforce_input_nowrap_atoms(node, &mut out);

        if let Some(hit_map) = state.hit_map.as_deref_mut()
            && let Some(hit_row_offset) = state.hit_row_offset.as_deref_mut()
        {
            let composed_lines = Layout::compose(&out.lines, state.compose_width.max(1));
            let pointer_rows = node
                .pointer_rows(ctx)
                .into_iter()
                .map(|entry| (entry.rendered_row, entry))
                .collect::<HashMap<u16, PointerRowMap>>();
            for (local_row, line) in composed_lines.iter().enumerate() {
                let local_row_u16 = local_row.min(u16::MAX as usize) as u16;
                let row = hit_row_offset.saturating_add(local_row_u16);
                let width = Layout::line_width(line.as_slice()).min(u16::MAX as usize) as u16;
                if pointer_rows.is_empty() {
                    let local_col_offset = if local_row == 0 { label_offset } else { 0 };
                    hit_map.push_node_row(
                        node.id(),
                        row,
                        local_row_u16,
                        state.hit_col_start,
                        state.hit_col_start.saturating_add(width),
                        local_col_offset,
                    );
                } else if let Some(PointerRowMap {
                    local_row,
                    local_col_offset,
                    local_semantic,
                    ..
                }) = pointer_rows.get(&local_row_u16)
                {
                    hit_map.push_node_row_with_semantic(
                        node.id(),
                        row,
                        state.hit_col_start,
                        state.hit_col_start.saturating_add(width),
                        HitLocal::row(*local_row)
                            .with_col_offset(*local_col_offset)
                            .with_semantic(*local_semantic),
                    );
                }
            }
            *hit_row_offset =
                hit_row_offset.saturating_add(composed_lines.len().min(u16::MAX as usize) as u16);
        }

        capture_node_focus_cursor(node, ctx, state, options, label_offset);
        *state.row_offset = (*state.row_offset).saturating_add(out.lines.len() as u16);
        if options.collect_sticky
            && let Some(sticky) = state.sticky.as_deref_mut()
            && !out.sticky.is_empty()
        {
            sticky.extend(out.sticky);
        }
        state.lines.extend(out.lines);
    }
}

fn capture_node_focus_cursor(
    node: &Node,
    ctx: &RenderContext,
    state: &mut DrawNodesState<'_>,
    options: DrawNodesOptions,
    label_offset: u16,
) {
    if !options.track_cursor
        || ctx
            .focused_id
            .as_deref()
            .is_none_or(|focused| focused != node.id())
    {
        return;
    }

    if state.focus_anchor.is_none() {
        *state.focus_anchor = Some(*state.row_offset);
    }
    if state.cursor.is_some() {
        return;
    }

    let available_width = ctx.terminal_size.width.saturating_sub(label_offset);
    let Some(local_cursor) = node.cursor_pos_with_width(available_width) else {
        return;
    };
    *state.cursor = Some(CursorPos {
        col: local_cursor.col.saturating_add(label_offset),
        row: (*state.row_offset).saturating_add(local_cursor.row),
    });
    *state.cursor_visible = node.cursor_visible();
}

pub(crate) fn register_block_selection_ranges(
    hit_map: &mut FrameHitMap,
    lines: &[SpanLine],
    col_start: u16,
) {
    for (row, line) in lines.iter().enumerate() {
        let row = row.min(u16::MAX as usize) as u16;
        let width = Layout::line_width(line.as_slice()).min(u16::MAX as usize) as u16;
        if width <= col_start {
            continue;
        }
        hit_map.push_selection_range(row, col_start, width);
    }
}

fn input_label_prefix(node: &Node, focused_id: Option<&str>) -> (Option<Vec<Span>>, u16) {
    let Node::Input(widget) = node else {
        return (None, 0);
    };

    let label = widget.label();
    if label.is_empty() {
        return (None, 0);
    }

    let label_style = if focused_id.is_some_and(|id| id == widget.id()) {
        Style::new().color(Color::White)
    } else {
        Style::default()
    };
    let prefix = vec![Span::styled(format!("{label}: "), label_style).no_wrap()];
    let offset = text_display_width(label)
        .saturating_add(2)
        .min(u16::MAX as usize) as u16;

    (Some(prefix), offset)
}

fn apply_input_validation_overlay(node: &Node, ctx: &RenderContext, out: &mut DrawOutput) {
    if !matches!(node, Node::Input(_)) {
        return;
    }

    if let Some(error) = ctx.visible_errors.get(node.id()) {
        let error_span = Span::styled(
            format!("✗ {}", error),
            Style::new().color(Color::Red).bold(),
        )
        .no_wrap();
        if let Some(first) = out.lines.first_mut() {
            *first = vec![error_span];
        } else {
            out.lines.push(vec![error_span]);
        }
        return;
    }

    if ctx.invalid_hidden.contains(node.id()) {
        for line in out.lines.iter_mut() {
            for span in line.iter_mut() {
                span.style.color = Some(Color::Red);
            }
        }
    }
}

fn enforce_input_nowrap_atoms(node: &Node, out: &mut DrawOutput) {
    if !matches!(node, Node::Input(_)) {
        return;
    }

    for line in &mut out.lines {
        for (idx, span) in line.iter_mut().enumerate() {
            span.wrap_mode = WrapMode::NoWrap;
            span.no_wrap_join_prev = idx > 0;
        }
    }
}
