use crate::ui::span::{SpanLine, WrapMode};

pub struct Layout;

impl Layout {
    pub fn compose(lines: &[SpanLine], width: u16) -> Vec<SpanLine> {
        if width == 0 {
            return Vec::new();
        }

        let mut out: Vec<SpanLine> = Vec::new();

        for line in lines {
            let mut current: SpanLine = Vec::new();
            let mut current_width = 0usize;

            for span in line {
                match span.wrap_mode {
                    WrapMode::NoWrap => {
                        if current_width > 0
                            && current_width + text_width(&span.text) > width as usize
                        {
                            out.push(current);
                            current = Vec::new();
                            current_width = 0;
                        }
                        current_width = current_width.saturating_add(text_width(&span.text));
                        current.push(span.clone());
                    }
                    WrapMode::Wrap => {
                        let mut rest = span.text.as_str();
                        while !rest.is_empty() {
                            if current_width >= width as usize {
                                out.push(current);
                                current = Vec::new();
                                current_width = 0;
                            }

                            let remaining = (width as usize).saturating_sub(current_width);
                            if remaining == 0 {
                                out.push(current);
                                current = Vec::new();
                                current_width = 0;
                                continue;
                            }

                            let (left, tail) = split_at_width(rest, remaining);
                            let mut piece = span.clone();
                            piece.text = left.to_string();
                            current_width = current_width.saturating_add(text_width(&piece.text));
                            current.push(piece);

                            rest = tail;
                            if !rest.is_empty() {
                                out.push(current);
                                current = Vec::new();
                                current_width = 0;
                            }
                        }
                    }
                }
            }

            out.push(current);
        }

        out
    }
}

fn text_width(s: &str) -> usize {
    s.chars().count()
}

fn split_at_width(s: &str, max: usize) -> (&str, &str) {
    if max == 0 {
        return ("", s);
    }

    let mut count = 0usize;
    let mut idx = s.len();
    for (byte_idx, _) in s.char_indices() {
        if count == max {
            idx = byte_idx;
            break;
        }
        count += 1;
    }

    if count < max {
        (s, "")
    } else {
        s.split_at(idx)
    }
}
