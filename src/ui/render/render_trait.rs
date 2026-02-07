use crate::ui::render::RenderLine;

#[derive(Debug, Clone, Copy)]
pub struct RenderCursor {
    pub line: usize,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub lines: Vec<RenderLine>,
    pub cursor: Option<RenderCursor>,
}

pub trait Render {
    fn render(&self, ctx: &crate::ui::render::RenderContext<'_>) -> RenderOutput;
}

impl RenderOutput {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            cursor: None,
        }
    }

    pub fn from_lines(lines: Vec<RenderLine>) -> Self {
        Self {
            lines,
            cursor: None,
        }
    }

    pub fn from_line(line: RenderLine) -> Self {
        Self::from_lines(vec![line])
    }

    pub fn with_cursor(mut self, line: usize, offset: usize) -> Self {
        self.cursor = Some(RenderCursor { line, offset });
        self
    }

    pub fn append(&mut self, mut other: RenderOutput) {
        if let Some(cursor) = other.cursor.take() {
            if self.cursor.is_none() {
                let line_offset = self.lines.len();
                self.cursor = Some(RenderCursor {
                    line: cursor.line + line_offset,
                    offset: cursor.offset,
                });
            }
        }
        self.lines.extend(other.lines);
    }
}
