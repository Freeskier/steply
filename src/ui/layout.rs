use crate::ui::span::{Span, SpanLine};
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

            for span in line {
                let mut rest = span.text.as_str();
                while !rest.is_empty() {
                    if current_width >= max_width {
                        push_line(&mut out, &mut current, &mut current_width);
                    }

                    let remaining = max_width.saturating_sub(current_width);
                    if remaining == 0 {
                        push_line(&mut out, &mut current, &mut current_width);
                        continue;
                    }

                    let (left, tail) = split_prefix_at_display_width(rest, remaining);
                    let piece_width = text_display_width(left);

                    map_cursor_in_segment(
                        cursor_target,
                        source_row,
                        source_col,
                        piece_width,
                        out.len(),
                        current_width,
                        &mut mapped_cursor,
                    );

                    let mut piece = span.clone();
                    piece.text = left.to_string();
                    current_width = current_width.saturating_add(piece_width);
                    current.push(piece);
                    source_col = source_col.saturating_add(piece_width);

                    rest = tail;
                    if !rest.is_empty() {
                        push_line(&mut out, &mut current, &mut current_width);
                    }
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
