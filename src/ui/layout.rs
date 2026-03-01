use crate::ui::span::{Span, SpanLine, WrapMode};
use crate::ui::text::{split_prefix_at_display_width, text_display_width};

pub struct Layout;

#[derive(Debug, Clone)]
pub struct RenderBlock {
    pub start_col: u16,
    pub end_col: Option<u16>,
    pub lines: Vec<SpanLine>,
}

#[derive(Debug, Clone)]
pub struct LineContinuation {
    pub first_prefix: SpanLine,
    pub next_prefix: SpanLine,
}

impl Layout {
    pub fn compose(lines: &[SpanLine], width: u16) -> Vec<SpanLine> {
        Self::compose_with_cursor(lines, width, None).0
    }

    pub fn compose_with_cursor(
        lines: &[SpanLine],
        width: u16,
        cursor: Option<(usize, usize)>,
    ) -> (Vec<SpanLine>, Option<(usize, usize)>) {
        if width == 0 {
            return (Vec::new(), None);
        }

        let mut out: Vec<SpanLine> = Vec::new();
        let mut mapped_cursor: Option<(usize, usize)> = None;
        let max_width = width as usize;
        let cursor_target = cursor;
        let mut current: SpanLine = Vec::new();
        let mut current_width = 0usize;

        for (source_row, line) in lines.iter().enumerate() {
            let mut source_col = 0usize;
            let mut compose_state = ComposeState {
                source_row,
                source_col: &mut source_col,
                max_width,
                out: &mut out,
                current: &mut current,
                current_width: &mut current_width,
                cursor_target,
                mapped_cursor: &mut mapped_cursor,
            };

            for unit in compose_units(line.as_slice()) {
                match unit {
                    ComposeUnit::WrapSpan(span) => compose_wrap_span(span, &mut compose_state),
                    ComposeUnit::NoWrapRun(run) => compose_nowrap_run(run, &mut compose_state),
                }
            }

            if mapped_cursor.is_none()
                && let Some((target_row, target_col)) = cursor_target
                && target_row == source_row
                && target_col >= source_col
            {
                mapped_cursor = Some((out.len(), current_width));
            }

            push_line(&mut out, &mut current, &mut current_width);
        }

        (out, mapped_cursor)
    }

    pub fn compose_block(
        block: &RenderBlock,
        viewport_width: u16,
        continuation: Option<&LineContinuation>,
    ) -> Vec<SpanLine> {
        if viewport_width == 0 || block.lines.is_empty() {
            return Vec::new();
        }

        let start = block.start_col.min(viewport_width);
        let end = block.end_col.unwrap_or(viewport_width).min(viewport_width);
        if end <= start {
            return Vec::new();
        }
        let block_width = end.saturating_sub(start);
        if block_width == 0 {
            return Vec::new();
        }

        let mut out = if let Some(continuation) = continuation {
            let (first_prefix, next_prefix, prefix_width) = normalize_prefixes(
                continuation.first_prefix.as_slice(),
                continuation.next_prefix.as_slice(),
            );
            let content_width = block_width.saturating_sub(prefix_width as u16).max(1);
            let mut composed = Vec::<SpanLine>::new();

            for source_line in &block.lines {
                let wrapped = Self::compose(std::slice::from_ref(source_line), content_width);
                for (idx, mut line) in wrapped.into_iter().enumerate() {
                    let mut prefixed = if idx == 0 {
                        first_prefix.clone()
                    } else {
                        next_prefix.clone()
                    };
                    prefixed.append(&mut line);
                    composed.push(prefixed);
                }
            }
            composed
        } else {
            Self::compose(block.lines.as_slice(), block_width)
        };

        if start > 0 {
            let indent = " ".repeat(start as usize);
            for line in &mut out {
                let mut prefixed = vec![Span::new(indent.clone()).no_wrap()];
                prefixed.append(line);
                *line = prefixed;
            }
        }

        out
    }

    pub fn line_width(line: &[Span]) -> usize {
        spans_width(line)
    }

    pub fn fit_line(line: &[Span], width: u16) -> SpanLine {
        if width == 0 {
            return Vec::new();
        }

        let mut fitted = Self::compose_block(
            &RenderBlock {
                start_col: 0,
                end_col: Some(width),
                lines: vec![line.to_vec()],
            },
            width,
            None,
        )
        .into_iter()
        .next()
        .unwrap_or_default();

        let used = Self::line_width(fitted.as_slice());
        let target = width as usize;
        if used < target {
            fitted.push(Span::new(" ".repeat(target - used)).no_wrap());
        }
        fitted
    }
}

fn normalize_prefixes(first: &[Span], next: &[Span]) -> (SpanLine, SpanLine, usize) {
    let first_width = spans_width(first);
    let next_width = spans_width(next);
    let target_width = first_width.max(next_width);

    (
        pad_prefix(first, target_width),
        pad_prefix(next, target_width),
        target_width,
    )
}

fn pad_prefix(prefix: &[Span], target_width: usize) -> SpanLine {
    let width = spans_width(prefix);
    let mut out = prefix.to_vec();
    if width < target_width {
        out.push(Span::new(" ".repeat(target_width - width)).no_wrap());
    }
    out
}

fn spans_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|span| text_display_width(span.text.as_str()))
        .sum()
}

fn push_line(out: &mut Vec<SpanLine>, current: &mut SpanLine, current_width: &mut usize) {
    out.push(std::mem::take(current));
    *current_width = 0;
}

fn map_cursor_in_segment(
    cursor_target: Option<(usize, usize)>,
    source_row: usize,
    source_col: usize,
    segment_width: usize,
    out_row: usize,
    out_col: usize,
    mapped_cursor: &mut Option<(usize, usize)>,
) {
    if mapped_cursor.is_some() {
        return;
    }

    let Some((target_row, target_col)) = cursor_target else {
        return;
    };

    if target_row != source_row {
        return;
    }

    let segment_end = source_col.saturating_add(segment_width);
    if target_col <= segment_end {
        let delta = target_col.saturating_sub(source_col).min(segment_width);
        *mapped_cursor = Some((out_row, out_col.saturating_add(delta)));
    }
}

fn continues_nowrap_run(prev: &Span, current: &Span) -> bool {
    matches!(prev.wrap_mode, WrapMode::NoWrap)
        && matches!(current.wrap_mode, WrapMode::NoWrap)
        && current.no_wrap_join_prev
}

#[derive(Debug, Clone)]
enum ComposeUnit {
    WrapSpan(Span),
    NoWrapRun(SpanLine),
}

fn compose_units(line: &[Span]) -> Vec<ComposeUnit> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    while idx < line.len() {
        let span = &line[idx];
        if !matches!(span.wrap_mode, WrapMode::NoWrap) {
            out.push(ComposeUnit::WrapSpan(span.clone()));
            idx = idx.saturating_add(1);
            continue;
        }

        let mut run = vec![span.clone()];
        idx = idx.saturating_add(1);
        while idx < line.len() && continues_nowrap_run(run.last().unwrap_or(span), &line[idx]) {
            run.push(line[idx].clone());
            idx = idx.saturating_add(1);
        }
        out.push(ComposeUnit::NoWrapRun(run));
    }
    out
}

struct ComposeState<'a> {
    source_row: usize,
    source_col: &'a mut usize,
    max_width: usize,
    out: &'a mut Vec<SpanLine>,
    current: &'a mut SpanLine,
    current_width: &'a mut usize,
    cursor_target: Option<(usize, usize)>,
    mapped_cursor: &'a mut Option<(usize, usize)>,
}

fn compose_wrap_span(span: Span, state: &mut ComposeState<'_>) {
    let mut rest = span.text.as_str();
    while !rest.is_empty() {
        if *state.current_width >= state.max_width {
            push_line(state.out, state.current, state.current_width);
        }

        let remaining = state.max_width.saturating_sub(*state.current_width);
        if remaining == 0 {
            push_line(state.out, state.current, state.current_width);
            continue;
        }

        let (left, tail) = split_prefix_at_display_width(rest, remaining);
        let piece_width = text_display_width(left);

        map_cursor_in_segment(
            state.cursor_target,
            state.source_row,
            *state.source_col,
            piece_width,
            state.out.len(),
            *state.current_width,
            state.mapped_cursor,
        );

        let mut piece = span.clone();
        piece.text = left.to_string();
        *state.current_width = state.current_width.saturating_add(piece_width);
        state.current.push(piece);
        *state.source_col = state.source_col.saturating_add(piece_width);

        rest = tail;
        if !rest.is_empty() {
            push_line(state.out, state.current, state.current_width);
        }
    }
}

fn compose_nowrap_run(mut run: SpanLine, state: &mut ComposeState<'_>) {
    while !run.is_empty() {
        if *state.current_width >= state.max_width {
            push_line(state.out, state.current, state.current_width);
        }

        let remaining = state.max_width.saturating_sub(*state.current_width);
        if remaining == 0 {
            push_line(state.out, state.current, state.current_width);
            continue;
        }

        let run_width = spans_width(run.as_slice());
        if run_width <= remaining {
            map_cursor_in_segment(
                state.cursor_target,
                state.source_row,
                *state.source_col,
                run_width,
                state.out.len(),
                *state.current_width,
                state.mapped_cursor,
            );
            *state.current_width = state.current_width.saturating_add(run_width);
            *state.source_col = state.source_col.saturating_add(run_width);
            state.current.extend(run);
            break;
        }

        if *state.current_width > 0 {
            push_line(state.out, state.current, state.current_width);
            continue;
        }

        let (prefix, prefix_width, rest) =
            take_spans_prefix_display_width(run.as_slice(), remaining);
        if prefix.is_empty() {
            break;
        }
        map_cursor_in_segment(
            state.cursor_target,
            state.source_row,
            *state.source_col,
            prefix_width,
            state.out.len(),
            *state.current_width,
            state.mapped_cursor,
        );
        *state.current_width = state.current_width.saturating_add(prefix_width);
        *state.source_col = state.source_col.saturating_add(prefix_width);
        state.current.extend(prefix);
        run = rest;
        if !run.is_empty() {
            push_line(state.out, state.current, state.current_width);
        }
    }
}

fn take_spans_prefix_display_width(
    spans: &[Span],
    max_width: usize,
) -> (SpanLine, usize, SpanLine) {
    if max_width == 0 || spans.is_empty() {
        return (Vec::new(), 0, spans.to_vec());
    }

    let mut taken = Vec::new();
    let mut rest = Vec::new();
    let mut used = 0usize;
    let mut split_done = false;

    for span in spans {
        if split_done {
            rest.push(span.clone());
            continue;
        }

        let span_width = text_display_width(span.text.as_str());
        if used.saturating_add(span_width) <= max_width {
            taken.push(span.clone());
            used = used.saturating_add(span_width);
            continue;
        }

        let remaining = max_width.saturating_sub(used);
        if remaining == 0 {
            rest.push(span.clone());
            split_done = true;
            continue;
        }

        let (left, right) = split_prefix_at_display_width(span.text.as_str(), remaining);
        if !left.is_empty() {
            let mut left_span = span.clone();
            left_span.text = left.to_string();
            used = used.saturating_add(text_display_width(left));
            taken.push(left_span);
        }
        if !right.is_empty() {
            let mut right_span = span.clone();
            right_span.text = right.to_string();
            rest.push(right_span);
        }
        split_done = true;
    }

    (taken, used, rest)
}
