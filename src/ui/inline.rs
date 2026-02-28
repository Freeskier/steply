use crate::ui::span::{Span, SpanLine, WrapMode};
use crate::ui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InlineWrap {
    #[default]
    Wrap,
    NoBreak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineGroup {
    pub style: Option<Style>,
    pub wrap: InlineWrap,
    pub children: Vec<Inline>,
}

impl InlineGroup {
    pub fn new(children: Vec<Inline>) -> Self {
        Self {
            style: None,
            wrap: InlineWrap::Wrap,
            children,
        }
    }

    pub fn no_break(children: Vec<Inline>) -> Self {
        Self {
            style: None,
            wrap: InlineWrap::NoBreak,
            children,
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    pub fn with_wrap(mut self, wrap: InlineWrap) -> Self {
        self.wrap = wrap;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(Span),
    Group(InlineGroup),
}

pub type InlineLine = Vec<Inline>;

impl Inline {
    pub fn text(span: Span) -> Self {
        Self::Text(span)
    }

    pub fn group(group: InlineGroup) -> Self {
        Self::Group(group)
    }
}

impl From<Span> for Inline {
    fn from(value: Span) -> Self {
        Self::Text(value)
    }
}

pub fn flatten_lines(lines: Vec<InlineLine>) -> Vec<SpanLine> {
    lines
        .into_iter()
        .map(|line| flatten_line(line.as_slice()))
        .collect()
}

pub fn flatten_line(line: &[Inline]) -> SpanLine {
    let mut out = Vec::<Span>::new();
    for node in line {
        flatten_node(node, Style::default(), &mut out);
    }
    out
}

fn flatten_node(node: &Inline, inherited_style: Style, out: &mut SpanLine) {
    match node {
        Inline::Text(span) => {
            let mut piece = span.clone();
            piece.style = inherited_style.merge(span.style);
            out.push(piece);
        }
        Inline::Group(group) => {
            let merged_style = group
                .style
                .map(|style| inherited_style.merge(style))
                .unwrap_or(inherited_style);
            let start = out.len();
            for child in group.children.as_slice() {
                flatten_node(child, merged_style, out);
            }
            if matches!(group.wrap, InlineWrap::NoBreak) {
                for (idx, span) in out[start..].iter_mut().enumerate() {
                    span.wrap_mode = WrapMode::NoWrap;
                    span.no_wrap_join_prev = idx > 0;
                }
            }
        }
    }
}
