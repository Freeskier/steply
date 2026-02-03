use crate::frame::Frame;
use crate::span::{Span, Wrap};

#[derive(Clone, Debug, Default)]
pub struct Layout {
    margin: usize,
}

impl Layout {
    pub fn new() -> Self {
        Self { margin: 0 }
    }

    pub fn with_margin(mut self, margin: usize) -> Self {
        self.margin = margin;
        self
    }

    pub fn compose_spans_with_cursor<I>(
        &self,
        spans_list: I,
        width: u16,
    ) -> (Frame, Option<(usize, usize)>)
    where
        I: IntoIterator<Item = (Vec<Span>, Option<usize>)>,
    {
        let mut ctx = LayoutContext::new(width as usize, self.margin);
        let mut cursor: Option<(usize, usize)> = None;
        let mut line_idx = 0usize;

        for (spans, cursor_offset) in spans_list {
            if let Some(offset) = cursor_offset {
                if cursor.is_none() {
                    let (row_offset, col) =
                        cursor_position_in_spans(&spans, width as usize, offset);
                    cursor = Some((col, line_idx + row_offset));
                }
            }

            line_idx += wrapped_line_count(&spans, width as usize);
            ctx.place_spans(spans);
        }

        (ctx.finish(), cursor)
    }
}

struct LayoutContext {
    frame: Frame,
    width: usize,
    current_width: usize,
}

impl LayoutContext {
    fn new(width: usize, margin: usize) -> Self {
        let width = width.saturating_sub(margin);
        let mut frame = Frame::new();
        frame.ensure_line();
        Self {
            frame,
            width,
            current_width: 0,
        }
    }

    fn place_spans(&mut self, spans: Vec<Span>) {
        for span in spans {
            if span.text() == "\n" {
                self.new_line();
                continue;
            }
            self.place_span(span);
        }
        self.new_line();
    }

    fn place_span(&mut self, span: Span) {
        if self.width == 0 || span.width() == 0 {
            return;
        }

        match span.wrap() {
            Wrap::No => self.place_no_wrap(span),
            Wrap::Yes => self.place_wrap(span),
        }
    }

    fn place_no_wrap(&mut self, span: Span) {
        let span_width = span.width();
        if self.current_width > 0 && span_width > self.available_width() {
            self.new_line();
        }

        let (head, _) = if span_width > self.width {
            span.split_at_width(self.width)
        } else {
            (span, None)
        };

        self.push_span(head);
    }

    fn place_wrap(&mut self, mut span: Span) {
        while span.width() > 0 {
            if self.current_width >= self.width {
                self.new_line();
            }

            let available = self.available_width();
            if span.width() <= available {
                self.push_span(span);
                return;
            }

            let (head, tail) = span.split_at_width(available);
            if head.width() > 0 {
                self.push_span(head);
            }
            self.new_line();

            match tail {
                Some(rest) => span = rest,
                None => return,
            }
        }
    }

    fn push_span(&mut self, span: Span) {
        let w = span.width();
        self.frame.current_line_mut().push(span);
        self.current_width += w;
    }

    fn new_line(&mut self) {
        self.frame.new_line();
        self.current_width = 0;
    }

    fn available_width(&self) -> usize {
        self.width.saturating_sub(self.current_width)
    }

    fn finish(mut self) -> Frame {
        self.frame.trim_trailing_empty();
        self.frame
    }
}

fn wrapped_line_count(spans: &[Span], width: usize) -> usize {
    if width == 0 {
        return 1;
    }

    let mut lines = 1usize;
    let mut current_width = 0usize;

    for span in spans {
        if span.text() == "\n" {
            lines += 1;
            current_width = 0;
            continue;
        }

        let span_width = span.width();
        if span_width == 0 {
            continue;
        }

        match span.wrap() {
            Wrap::No => {
                let available = width.saturating_sub(current_width);
                if current_width > 0 && span_width > available {
                    lines += 1;
                    current_width = 0;
                }
                let head_width = span_width.min(width);
                current_width += head_width;
            }
            Wrap::Yes => {
                let mut remaining = span_width;
                while remaining > 0 {
                    if current_width >= width {
                        lines += 1;
                        current_width = 0;
                    }
                    let available = width - current_width;
                    if remaining <= available {
                        current_width += remaining;
                        remaining = 0;
                    } else {
                        remaining -= available;
                        lines += 1;
                        current_width = 0;
                    }
                }
            }
        }
    }

    lines.max(1)
}

fn cursor_position_in_spans(spans: &[Span], width: usize, cursor_offset: usize) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let mut row = 0usize;
    let mut current_width = 0usize;
    let mut remaining = cursor_offset;

    for span in spans {
        if remaining == 0 {
            return (row, current_width);
        }

        if span.text() == "\n" {
            row += 1;
            current_width = 0;
            continue;
        }

        let span_width = span.width();
        if span_width == 0 {
            continue;
        }

        match span.wrap() {
            Wrap::No => {
                let available = width.saturating_sub(current_width);
                if current_width > 0 && span_width > available {
                    row += 1;
                    current_width = 0;
                }

                let head_width = span_width.min(width);
                if remaining <= head_width {
                    return (row, current_width + remaining);
                }
                remaining -= head_width;
                current_width += head_width;
            }
            Wrap::Yes => {
                let mut part = span.clone();
                loop {
                    if current_width >= width {
                        row += 1;
                        current_width = 0;
                    }
                    let available = width - current_width;
                    if part.width() <= available {
                        let part_width = part.width();
                        if remaining <= part_width {
                            return (row, current_width + remaining);
                        }
                        remaining -= part_width;
                        current_width += part_width;
                        break;
                    }

                    let (head, tail) = part.split_at_width(available);
                    let head_width = head.width();
                    if head_width > 0 {
                        if remaining <= head_width {
                            return (row, current_width + remaining);
                        }
                        remaining -= head_width;
                    }
                    row += 1;
                    current_width = 0;

                    if let Some(rest) = tail {
                        part = rest;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    (row, current_width)
}
