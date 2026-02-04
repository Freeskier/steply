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
            let (line_count, cursor_pos) = scan_spans(&spans, width as usize, cursor_offset);

            if cursor.is_none() {
                if let Some((row_offset, col)) = cursor_pos {
                    cursor = Some((col, line_idx + row_offset));
                }
            }

            line_idx += line_count;
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

fn scan_spans(
    spans: &[Span],
    width: usize,
    cursor_offset: Option<usize>,
) -> (usize, Option<(usize, usize)>) {
    if width == 0 {
        return (1, cursor_offset.map(|_| (0, 0)));
    }

    let mut line_count = 1usize;
    let mut row = 0usize;
    let mut current_width = 0usize;
    let mut remaining_cursor = cursor_offset;
    let mut cursor_pos: Option<(usize, usize)> = None;

    for span in spans {
        if remaining_cursor == Some(0) && cursor_pos.is_none() {
            cursor_pos = Some((row, current_width));
            remaining_cursor = None;
        }

        if span.text() == "\n" {
            line_count += 1;
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
                    line_count += 1;
                    row += 1;
                    current_width = 0;
                }

                let head_width = span_width.min(width);

                if let Some(remaining) = remaining_cursor {
                    if remaining <= head_width {
                        cursor_pos = Some((row, current_width + remaining));
                        remaining_cursor = None;
                    } else {
                        remaining_cursor = Some(remaining - head_width);
                    }
                }

                current_width += head_width;
            }
            Wrap::Yes => {
                let mut remaining = span_width;
                while remaining > 0 {
                    if current_width >= width {
                        line_count += 1;
                        row += 1;
                        current_width = 0;
                    }

                    let available = width - current_width;
                    if remaining <= available {
                        if let Some(rem) = remaining_cursor {
                            if rem <= remaining {
                                cursor_pos = Some((row, current_width + rem));
                                remaining_cursor = None;
                            } else {
                                remaining_cursor = Some(rem - remaining);
                            }
                        }

                        current_width += remaining;
                        remaining = 0;
                    } else {
                        if let Some(rem) = remaining_cursor {
                            if rem <= available {
                                cursor_pos = Some((row, current_width + rem));
                                remaining_cursor = None;
                            } else {
                                remaining_cursor = Some(rem - available);
                            }
                        }

                        remaining -= available;
                        line_count += 1;
                        row += 1;
                        current_width = 0;
                    }
                }
            }
        }
    }

    if cursor_pos.is_none() && cursor_offset.is_some() {
        cursor_pos = Some((row, current_width));
    }

    (line_count.max(1), cursor_pos)
}
