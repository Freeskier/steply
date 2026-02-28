use super::InlineState;
use crate::ui::span::{Span, SpanLine, WrapMode};
use crate::ui::style::{Color, Strike};
use crate::ui::text::char_display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirtyRange {
    pub start: u16,
    pub end_inclusive: u16,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DirtyRows {
    rows: Vec<u16>,
    ranges: Vec<DirtyRange>,
}

impl DirtyRows {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn ranges(&self) -> &[DirtyRange] {
        self.ranges.as_slice()
    }

    fn from_rows(rows: Vec<u16>) -> Self {
        let mut ranges = Vec::<DirtyRange>::new();
        let mut iter = rows.iter().copied();
        let Some(mut start) = iter.next() else {
            return Self { rows, ranges };
        };
        let mut end = start;
        for row in iter {
            if row == end.saturating_add(1) {
                end = row;
                continue;
            }
            ranges.push(DirtyRange {
                start,
                end_inclusive: end,
            });
            start = row;
            end = row;
        }
        ranges.push(DirtyRange {
            start,
            end_inclusive: end,
        });
        Self { rows, ranges }
    }
}

pub(super) fn compute_dirty_rows(
    prev_lines: Option<&[SpanLine]>,
    prev_offset: usize,
    next_lines: &[SpanLine],
    next_offset: usize,
    row_count: usize,
    force_all: bool,
) -> DirtyRows {
    if force_all || prev_lines.is_none() {
        return DirtyRows::from_rows(
            (0..row_count.min(u16::MAX as usize))
                .map(|row| row as u16)
                .collect(),
        );
    }

    let prev_lines = prev_lines.expect("checked above");
    let mut dirty = Vec::new();
    for row in 0..row_count {
        let prev_line = prev_lines.get(prev_offset + row);
        let next_line = next_lines.get(next_offset + row);
        if prev_line != next_line {
            if row > u16::MAX as usize {
                break;
            }
            dirty.push(row as u16);
        }
    }
    DirtyRows::from_rows(dirty)
}

pub(super) fn estimate_self_reflow_cursor_delta(inline: &InlineState, new_width: u16) -> i32 {
    if !inline.has_rendered_once || inline.last_drawn_count == 0 {
        return 0;
    }
    let old_width = inline.last_rendered_size.width;
    if old_width == 0 || new_width == 0 {
        return 0;
    }

    let old_skip = inline.last_skip;
    let visible_lines = &inline.last_frame[old_skip..];
    if visible_lines.is_empty() {
        return 0;
    }

    let cursor = match inline.last_rendered_cursor {
        Some(cursor) => cursor,
        None => return 0,
    };
    let cursor_abs_row = cursor.row as usize;
    if cursor_abs_row < old_skip {
        return 0;
    }
    let cursor_visible_row = inline
        .last_cursor_row
        .min(visible_lines.len().saturating_sub(1) as u16) as usize;

    let new_width_usize = new_width as usize;
    let mut new_row = 0usize;
    for line in visible_lines.iter().take(cursor_visible_row) {
        let width = rendered_line_width(line, old_width);
        new_row = new_row.saturating_add(wrapped_rows(width, new_width_usize));
    }

    if visible_lines.get(cursor_visible_row).is_none() {
        return 0;
    }

    let prefix = inline.last_cursor_col.min(old_width.saturating_sub(1)) as usize;
    new_row = new_row.saturating_add(prefix / new_width_usize);

    new_row as i32 - cursor_visible_row as i32
}

fn rendered_line_width(line: &SpanLine, old_width: u16) -> usize {
    let render_width = if old_width > 1 {
        (old_width - 1) as usize
    } else {
        old_width as usize
    };
    if render_width == 0 {
        return 0;
    }

    let mut used = 0usize;
    for span in line {
        if used >= render_width {
            break;
        }
        let available = render_width.saturating_sub(used);
        let mut span_used = 0usize;
        for ch in span.text.chars().filter(|ch| !matches!(ch, '\n' | '\r')) {
            let ch_width = char_display_width(ch);
            if span_used.saturating_add(ch_width) > available {
                break;
            }
            span_used = span_used.saturating_add(ch_width);
        }
        used = used.saturating_add(span_used);
    }
    used
}

fn wrapped_rows(line_width: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if line_width == 0 {
        return 1;
    }
    (line_width.saturating_sub(1) / width).saturating_add(1)
}

pub(super) fn quick_frame_signature(lines: &[SpanLine]) -> u64 {
    let mut acc = 0xcbf29ce484222325u64 ^ (lines.len() as u64);
    for (row_idx, line) in lines.iter().enumerate() {
        acc = mix_sig(acc, row_idx as u64);
        acc = mix_sig(acc, line.len() as u64);
        for span in line {
            acc = mix_sig(acc, quick_span_signature(span));
        }
    }
    acc
}

fn quick_span_signature(span: &Span) -> u64 {
    let mut sig = 0x9e3779b97f4a7c15u64;
    let bytes = span.text.as_bytes();
    sig = mix_sig(sig, bytes.len() as u64);
    if !bytes.is_empty() {
        let head = bytes
            .iter()
            .take(4)
            .fold(0u64, |acc, b| (acc << 8) ^ (*b as u64));
        let tail = bytes
            .iter()
            .rev()
            .take(4)
            .fold(0u64, |acc, b| (acc << 8) ^ (*b as u64));
        sig = mix_sig(sig, head);
        sig = mix_sig(sig, tail);
    }
    sig = mix_sig(sig, color_sig(span.style.color));
    sig = mix_sig(sig, color_sig(span.style.background));
    sig = mix_sig(sig, if span.style.bold { 1 } else { 0 });
    sig = mix_sig(sig, strike_sig(span.style.strike));
    sig = mix_sig(
        sig,
        match span.wrap_mode {
            WrapMode::Wrap => 0,
            WrapMode::NoWrap => 1,
        },
    );
    sig
}

fn color_sig(color: Option<Color>) -> u64 {
    match color {
        None => 0,
        Some(Color::Reset) => 1,
        Some(Color::Black) => 2,
        Some(Color::DarkGrey) => 3,
        Some(Color::Red) => 4,
        Some(Color::Green) => 5,
        Some(Color::Yellow) => 6,
        Some(Color::Blue) => 7,
        Some(Color::Magenta) => 8,
        Some(Color::Cyan) => 9,
        Some(Color::White) => 10,
        Some(Color::Rgb(r, g, b)) => 11u64 << 32 | (r as u64) << 16 | (g as u64) << 8 | (b as u64),
    }
}

fn strike_sig(strike: Strike) -> u64 {
    match strike {
        Strike::Inherit => 0,
        Strike::On => 1,
        Strike::Off => 2,
    }
}

fn mix_sig(acc: u64, value: u64) -> u64 {
    let mixed = acc ^ value.wrapping_mul(0x517cc1b727220a95);
    mixed.rotate_left(13).wrapping_mul(0x100000001b3)
}
