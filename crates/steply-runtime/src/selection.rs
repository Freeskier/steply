use steply_core::terminal::{KeyModifiers, PointerButton, PointerEvent, PointerKind};
use steply_core::ui::hit_test::FrameHitMap;
use steply_core::ui::span::{Span, SpanLine};
use steply_core::ui::style::{Color, Style};
use steply_core::ui::text::{split_prefix_at_display_width, text_display_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: u16,
    pub col: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: SelectionPoint,
    pub end: SelectionPoint,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionState {
    anchor: Option<SelectionPoint>,
    head: Option<SelectionPoint>,
    pub pending_anchor: Option<SelectionPoint>,
    pub dragging: bool,
}

impl SelectionState {
    pub fn begin(&mut self, at: SelectionPoint) -> bool {
        let changed = self.anchor != Some(at) || self.head != Some(at) || !self.dragging;
        self.anchor = Some(at);
        self.head = Some(at);
        self.pending_anchor = None;
        self.dragging = true;
        changed
    }

    pub fn update(&mut self, at: SelectionPoint) -> bool {
        if !self.dragging {
            return false;
        }
        let changed = self.head != Some(at);
        self.head = Some(at);
        changed
    }

    pub fn end(&mut self, at: SelectionPoint) -> bool {
        if !self.dragging {
            return false;
        }
        let changed = self.head != Some(at) || self.dragging;
        self.head = Some(at);
        self.dragging = false;
        self.pending_anchor = None;
        changed
    }

    pub fn set_pending_anchor(&mut self, at: SelectionPoint) -> bool {
        let had_selection = self.anchor.is_some() || self.head.is_some() || self.dragging;
        self.anchor = None;
        self.head = None;
        self.dragging = false;
        self.pending_anchor = Some(at);
        had_selection
    }

    pub fn begin_from_pending_or(&mut self, at: SelectionPoint) -> bool {
        let anchor = self.pending_anchor.unwrap_or(at);
        self.begin(anchor)
    }

    pub fn range(&self) -> Option<SelectionRange> {
        let (Some(anchor), Some(head)) = (self.anchor, self.head) else {
            return None;
        };
        if anchor == head {
            return None;
        }
        let (start, end) = if (anchor.row, anchor.col) <= (head.row, head.col) {
            (anchor, head)
        } else {
            (head, anchor)
        };
        Some(SelectionRange { start, end })
    }
}

pub fn handle_selection_pointer(
    selection: &mut SelectionState,
    event: PointerEvent,
) -> (bool, bool) {
    let point = SelectionPoint {
        row: event.row,
        col: event.col,
    };
    match event.kind {
        PointerKind::Down(PointerButton::Left) => {
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                return (true, selection.begin(point));
            }
            let changed = selection.set_pending_anchor(point);
            (false, changed)
        }
        PointerKind::Drag(PointerButton::Left) => {
            if selection.dragging {
                return (true, selection.update(point));
            }
            if selection.pending_anchor.is_some()
                || event.modifiers.contains(KeyModifiers::SHIFT)
            {
                let mut changed = selection.begin_from_pending_or(point);
                changed |= selection.update(point);
                return (true, changed);
            }
            (false, false)
        }
        PointerKind::Up(PointerButton::Left) => {
            if selection.dragging {
                return (true, selection.end(point));
            }
            let changed = selection.pending_anchor.take().is_some();
            (false, changed)
        }
        _ => (false, false),
    }
}

pub fn apply_selection_highlight(
    hit_map: &FrameHitMap,
    lines: &mut [SpanLine],
    range: SelectionRange,
) {
    if lines.is_empty() {
        return;
    }

    let mut start_row = range.start.row as usize;
    let mut end_row = range.end.row as usize;
    if start_row >= lines.len() {
        return;
    }
    if end_row >= lines.len() {
        end_row = lines.len().saturating_sub(1);
    }
    if start_row > end_row {
        std::mem::swap(&mut start_row, &mut end_row);
    }

    for (row_idx, line) in lines
        .iter_mut()
        .enumerate()
        .take(end_row + 1)
        .skip(start_row)
    {
        let line_width = display_width_for_line(line.as_slice()) as u16;
        let row_start = if row_idx == start_row {
            range.start.col.min(line_width)
        } else {
            0
        };
        let row_end = if row_idx == end_row {
            range.end.col.min(line_width)
        } else {
            line_width
        };
        if row_end <= row_start {
            continue;
        }
        let selectable =
            selectable_ranges_for_row(hit_map, row_idx as u16, line_width);
        if selectable.is_empty() {
            continue;
        }
        for (sel_start, sel_end) in selectable {
            let start = row_start.max(sel_start).min(line_width);
            let end = row_end.min(sel_end).min(line_width);
            if end > start {
                highlight_line_range(line, start as usize, end as usize);
            }
        }
    }
}

pub fn extract_selected_text(
    hit_map: &FrameHitMap,
    frame_lines: &[SpanLine],
    range: SelectionRange,
) -> Option<String> {
    extract_selected_text_with_mode(hit_map, frame_lines, range, true)
        .or_else(|| extract_selected_text_with_mode(hit_map, frame_lines, range, false))
}

fn extract_selected_text_with_mode(
    hit_map: &FrameHitMap,
    frame_lines: &[SpanLine],
    range: SelectionRange,
    strict_hit_map: bool,
) -> Option<String> {
    if frame_lines.is_empty() {
        return None;
    }
    let start_row = range.start.row as usize;
    if start_row >= frame_lines.len() {
        return None;
    }
    let end_row = (range.end.row as usize).min(frame_lines.len() - 1);
    if end_row < start_row {
        return None;
    }

    let mut rows = Vec::<String>::new();
    for row_idx in start_row..=end_row {
        let line = &frame_lines[row_idx];
        let line_width = display_width_for_line(line.as_slice()) as u16;
        let row_start = if row_idx == start_row {
            range.start.col.min(line_width)
        } else {
            0
        };
        let row_end = if row_idx == end_row {
            range.end.col.min(line_width)
        } else {
            line_width
        };
        if row_end <= row_start {
            continue;
        }

        let selectable = selectable_ranges_for_row_mode(
            hit_map,
            row_idx as u16,
            line_width,
            strict_hit_map,
        );
        if selectable.is_empty() {
            continue;
        }
        let mut row_text = String::new();
        for (sel_start, sel_end) in selectable {
            let start = row_start.max(sel_start).min(line_width);
            let end = row_end.min(sel_end).min(line_width);
            if end > start {
                row_text.push_str(&extract_line_text_range(
                    line.as_slice(),
                    start as usize,
                    end as usize,
                ));
            }
        }
        if !row_text.is_empty() {
            rows.push(row_text);
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows.join("\n"))
    }
}

fn selection_highlight_style() -> Style {
    Style::new().background(Color::Blue)
}

fn display_width_for_line(line: &[Span]) -> usize {
    line.iter()
        .map(|span| text_display_width(span.text.as_str()))
        .sum()
}

fn selectable_ranges_for_row(hit_map: &FrameHitMap, row: u16, line_width: u16) -> Vec<(u16, u16)> {
    selectable_ranges_for_row_mode(hit_map, row, line_width, true)
}

fn selectable_ranges_for_row_mode(
    hit_map: &FrameHitMap,
    row: u16,
    line_width: u16,
    strict_hit_map: bool,
) -> Vec<(u16, u16)> {
    let from_hit_map = hit_map.row_ranges(row);
    if !from_hit_map.is_empty() {
        return from_hit_map;
    }
    if strict_hit_map && hit_map.has_any_ranges() {
        return Vec::new();
    }
    fallback_selectable_ranges(line_width)
}

fn fallback_selectable_ranges(line_width: u16) -> Vec<(u16, u16)> {
    if line_width == 0 {
        return Vec::new();
    }
    vec![(0, line_width)]
}

fn highlight_line_range(line: &mut SpanLine, start_col: usize, end_col: usize) {
    if end_col <= start_col || line.is_empty() {
        return;
    }

    let mut out = Vec::<Span>::with_capacity(line.len().saturating_mul(2));
    let mut col = 0usize;
    for span in line.iter() {
        let width = text_display_width(span.text.as_str());
        if width == 0 {
            out.push(span.clone());
            continue;
        }

        let span_start = col;
        let span_end = col.saturating_add(width);
        let sel_start = start_col.max(span_start);
        let sel_end = end_col.min(span_end);
        if sel_end <= sel_start {
            out.push(span.clone());
            col = span_end;
            continue;
        }

        let left_width = sel_start.saturating_sub(span_start);
        let mid_width = sel_end.saturating_sub(sel_start);
        let (left, tail) = split_prefix_at_display_width(span.text.as_str(), left_width);
        let (mid, right) = split_prefix_at_display_width(tail, mid_width);

        if !left.is_empty() {
            let mut piece = span.clone();
            piece.text = left.to_string();
            out.push(piece);
        }
        if !mid.is_empty() {
            let mut piece = span.clone();
            piece.text = mid.to_string();
            piece.style = span.style.merge(selection_highlight_style());
            out.push(piece);
        }
        if !right.is_empty() {
            let mut piece = span.clone();
            piece.text = right.to_string();
            out.push(piece);
        }

        col = span_end;
    }

    *line = out;
}

fn extract_line_text_range(line: &[Span], start_col: usize, end_col: usize) -> String {
    if end_col <= start_col || line.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut col = 0usize;
    for span in line {
        let width = text_display_width(span.text.as_str());
        if width == 0 {
            continue;
        }

        let span_start = col;
        let span_end = col.saturating_add(width);
        let sel_start = start_col.max(span_start);
        let sel_end = end_col.min(span_end);
        if sel_end <= sel_start {
            col = span_end;
            continue;
        }

        let left_width = sel_start.saturating_sub(span_start);
        let mid_width = sel_end.saturating_sub(sel_start);
        let (_, tail) = split_prefix_at_display_width(span.text.as_str(), left_width);
        let (mid, _) = split_prefix_at_display_width(tail, mid_width);
        out.push_str(mid);

        col = span_end;
    }

    out
}
