use crate::ui::span::{SpanLine, WrapMode};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct Layout;

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
                match span.wrap_mode {
                    WrapMode::NoWrap => {
                        // Hard-wrap long no-wrap spans instead of clipping them.
                        // This keeps full text visible when a single long token
                        // exceeds the available terminal width.
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

                            let (left, tail) = split_at_width(rest, remaining);
                            let piece_width = text_width(left);

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
                    WrapMode::Wrap => {
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

                            let (left, tail) = split_at_width(rest, remaining);
                            let piece_width = text_width(left);

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

fn text_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn split_at_width(s: &str, max: usize) -> (&str, &str) {
    if max == 0 {
        return ("", s);
    }

    let mut width = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width.saturating_add(ch_width) > max {
            if byte_idx == 0 {
                let next = ch.len_utf8();
                return s.split_at(next);
            }
            return s.split_at(byte_idx);
        }
        width = width.saturating_add(ch_width);
    }

    (s, "")
}
