use crate::terminal::CursorPos;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::ui::text::{split_prefix_at_display_width, text_display_width};

#[derive(Debug, Clone)]
pub struct ViewportLine {
    pub spans: SpanLine,
    pub cursor: Option<CursorPos>,
}

pub fn render_single_line(
    spans: &[Span],
    width: u16,
    follow_range: Option<(usize, usize)>,
    cursor_col: Option<usize>,
) -> ViewportLine {
    let width = width as usize;
    if width == 0 {
        return ViewportLine {
            spans: Vec::new(),
            cursor: None,
        };
    }

    let total_width = spans
        .iter()
        .map(|span| text_display_width(span.text.as_str()))
        .sum::<usize>();
    if total_width <= width {
        return ViewportLine {
            spans: normalize_nowrap_spans(spans.to_vec()),
            cursor: cursor_col.map(|col| CursorPos {
                col: col.min(u16::MAX as usize) as u16,
                row: 0,
            }),
        };
    }

    let offset = follow_range
        .map(|(start, end)| offset_for_range(total_width, width, start, end))
        .unwrap_or(0);
    let layout = layout_for(total_width, width, offset);
    let mut out = Vec::new();

    if layout.has_left_overflow {
        out.push(overflow_indicator());
    }

    out.extend(slice_spans(spans, offset, layout.content_width));

    if layout.has_right_overflow {
        out.push(overflow_indicator());
    }

    let cursor = cursor_col.map(|col| {
        let clipped = if col <= offset {
            usize::from(layout.has_left_overflow)
        } else {
            usize::from(layout.has_left_overflow)
                .saturating_add(col.saturating_sub(offset).min(layout.content_width))
        };
        CursorPos {
            col: clipped.min(u16::MAX as usize) as u16,
            row: 0,
        }
    });

    ViewportLine {
        spans: normalize_nowrap_spans(out),
        cursor,
    }
}

fn overflow_indicator() -> Span {
    Span::styled("…", Style::new().color(Color::DarkGrey)).no_wrap()
}

fn normalize_nowrap_spans(mut spans: SpanLine) -> SpanLine {
    for (index, span) in spans.iter_mut().enumerate() {
        span.wrap_mode = crate::ui::span::WrapMode::NoWrap;
        span.no_wrap_join_prev = index > 0;
    }
    spans
}

#[derive(Debug, Clone, Copy)]
struct LayoutState {
    content_width: usize,
    has_left_overflow: bool,
    has_right_overflow: bool,
}

fn layout_for(total_width: usize, width: usize, offset: usize) -> LayoutState {
    let has_left_overflow = offset > 0;
    let left_reserved = usize::from(has_left_overflow);
    let mut right_reserved = 0usize;

    loop {
        let content_width = width.saturating_sub(left_reserved + right_reserved).max(1);
        let has_right_overflow = offset.saturating_add(content_width) < total_width;
        let next_right_reserved = usize::from(has_right_overflow);
        if next_right_reserved == right_reserved {
            return LayoutState {
                content_width,
                has_left_overflow,
                has_right_overflow,
            };
        }
        right_reserved = next_right_reserved;
    }
}

fn max_content_width(width: usize) -> usize {
    if width <= 2 { 1 } else { width - 2 }
}

fn offset_for_range(total_width: usize, width: usize, start: usize, end: usize) -> usize {
    let target_end = end.max(start.saturating_add(1));
    let target_len = target_end.saturating_sub(start);
    if target_len >= max_content_width(width) {
        return offset_for_cursor(total_width, width, target_end.saturating_sub(1));
    }

    let max_offset = total_width.saturating_sub(1);
    for offset in 0..=max_offset {
        let layout = layout_for(total_width, width, offset);
        let visible_end = offset.saturating_add(layout.content_width);
        if start >= offset && target_end <= visible_end {
            return offset;
        }
    }

    offset_for_cursor(total_width, width, target_end.saturating_sub(1))
}

fn offset_for_cursor(total_width: usize, width: usize, cursor_col: usize) -> usize {
    let max_offset = total_width.saturating_sub(1);
    for offset in 0..=max_offset {
        let layout = layout_for(total_width, width, offset);
        let visible_end = offset.saturating_add(layout.content_width);
        if cursor_col >= offset && cursor_col < visible_end {
            return offset;
        }
    }
    max_offset
}

fn slice_spans(spans: &[Span], start_col: usize, width: usize) -> SpanLine {
    if width == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut consumed = 0usize;
    let end_col = start_col.saturating_add(width);

    for span in spans {
        let span_width = text_display_width(span.text.as_str());
        let span_end = consumed.saturating_add(span_width);
        if span_end <= start_col {
            consumed = span_end;
            continue;
        }
        if consumed >= end_col {
            break;
        }

        let local_start = start_col.saturating_sub(consumed);
        let local_width = end_col.saturating_sub(consumed.saturating_add(local_start));
        let piece = clip_text_range(span.text.as_str(), local_start, local_width);
        if !piece.is_empty() {
            let mut sliced = span.clone();
            sliced.text = piece;
            out.push(sliced);
        }
        consumed = span_end;
    }

    out
}

fn clip_text_range(text: &str, start_col: usize, width: usize) -> String {
    let (_, tail) = split_prefix_at_display_width(text, start_col);
    let (head, _) = split_prefix_at_display_width(tail, width);
    head.to_string()
}

#[cfg(test)]
#[path = "tests/horizontal_viewport.rs"]
mod tests;
