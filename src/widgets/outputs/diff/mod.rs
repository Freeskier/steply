use similar::{DiffOp, TextDiff};

use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::CursorNav;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};



#[derive(Clone)]
enum Side {
    Line { no: usize, text: String },
    Empty,
}

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    Context,
    Removed,
    Added,
    Changed,
}

#[derive(Clone)]
enum DiffRow {
    Line {
        left: Side,
        right: Side,
        kind: RowKind,
    },


    Gap { hidden: usize },
}



pub struct DiffOutput {
    base: WidgetBase,
    old: String,
    new: String,

    context: usize,
    rows: Vec<DiffRow>,
    nav: CursorNav,
}

impl DiffOutput {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        old: impl Into<String>,
        new: impl Into<String>,
    ) -> Self {
        let mut this = Self {
            base: WidgetBase::new(id, label),
            old: old.into(),
            new: new.into(),
            context: 3,
            rows: Vec::new(),
            nav: CursorNav::new(Some(20)),
        };
        this.rebuild();
        this
    }

    pub fn with_max_visible(mut self, n: usize) -> Self {
        self.nav.set_max_visible(n);
        self
    }

    pub fn set_texts(&mut self, old: impl Into<String>, new: impl Into<String>) {
        self.old = old.into();
        self.new = new.into();
        self.rebuild();
    }



    fn rebuild(&mut self) {
        self.rows = Self::build_rows(&self.old, &self.new, self.context);
        self.nav.clamp(self.rows.len());
    }

    fn build_rows(old: &str, new: &str, context: usize) -> Vec<DiffRow> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let diff = TextDiff::from_lines(old, new);
        let groups = diff.grouped_ops(context);

        let mut rows: Vec<DiffRow> = Vec::new();
        let mut prev_old_end = 0usize;

        for group in &groups {

            let group_old_start = group.first().map(|op| op.old_range().start).unwrap_or(0);
            let hidden = group_old_start.saturating_sub(prev_old_end);
            if hidden > 0 {
                rows.push(DiffRow::Gap { hidden });
            }

            for op in group {
                match op {
                    DiffOp::Equal {
                        old_index,
                        new_index,
                        len,
                    } => {
                        for i in 0..*len {
                            rows.push(DiffRow::Line {
                                left: Side::Line {
                                    no: old_index + i + 1,
                                    text: old_lines
                                        .get(old_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                right: Side::Line {
                                    no: new_index + i + 1,
                                    text: new_lines
                                        .get(new_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                kind: RowKind::Context,
                            });
                        }
                    }
                    DiffOp::Delete {
                        old_index, old_len, ..
                    } => {
                        for i in 0..*old_len {
                            rows.push(DiffRow::Line {
                                left: Side::Line {
                                    no: old_index + i + 1,
                                    text: old_lines
                                        .get(old_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                right: Side::Empty,
                                kind: RowKind::Removed,
                            });
                        }
                    }
                    DiffOp::Insert {
                        new_index, new_len, ..
                    } => {
                        for i in 0..*new_len {
                            rows.push(DiffRow::Line {
                                left: Side::Empty,
                                right: Side::Line {
                                    no: new_index + i + 1,
                                    text: new_lines
                                        .get(new_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                kind: RowKind::Added,
                            });
                        }
                    }
                    DiffOp::Replace {
                        old_index,
                        old_len,
                        new_index,
                        new_len,
                    } => {
                        let pairs = (*old_len).min(*new_len);

                        for i in 0..pairs {
                            rows.push(DiffRow::Line {
                                left: Side::Line {
                                    no: old_index + i + 1,
                                    text: old_lines
                                        .get(old_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                right: Side::Line {
                                    no: new_index + i + 1,
                                    text: new_lines
                                        .get(new_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                kind: RowKind::Changed,
                            });
                        }

                        for i in pairs..*old_len {
                            rows.push(DiffRow::Line {
                                left: Side::Line {
                                    no: old_index + i + 1,
                                    text: old_lines
                                        .get(old_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                right: Side::Empty,
                                kind: RowKind::Removed,
                            });
                        }

                        for i in pairs..*new_len {
                            rows.push(DiffRow::Line {
                                left: Side::Empty,
                                right: Side::Line {
                                    no: new_index + i + 1,
                                    text: new_lines
                                        .get(new_index + i)
                                        .copied()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                kind: RowKind::Added,
                            });
                        }
                    }
                }
            }

            prev_old_end = group
                .last()
                .map(|op| op.old_range().end)
                .unwrap_or(prev_old_end);
        }


        let trailing = old_lines.len().saturating_sub(prev_old_end);
        if trailing > 0 {
            rows.push(DiffRow::Gap { hidden: trailing });
        }

        rows
    }



    fn move_cursor(&mut self, delta: isize) {
        self.nav.move_by(delta, self.rows.len());
    }

    fn next_chunk(&mut self) {
        let start = self.nav.active() + 1;
        if start >= self.rows.len() {
            return;
        }
        if let Some(pos) = self.rows[start..].iter().position(|r| {
            matches!(r, DiffRow::Gap { .. })
                || matches!(r, DiffRow::Line { kind, .. } if *kind != RowKind::Context)
        }) {
            self.nav.set_active(start + pos, self.rows.len());
        }
    }

    fn prev_chunk(&mut self) {
        if self.nav.active() == 0 {
            return;
        }
        if let Some(pos) = self.rows[..self.nav.active()].iter().rposition(|r| {
            matches!(r, DiffRow::Gap { .. })
                || matches!(r, DiffRow::Line { kind, .. } if *kind != RowKind::Context)
        }) {
            self.nav.set_active(pos, self.rows.len());
        }
    }

    fn expand_gap(&mut self) {
        if !matches!(self.rows.get(self.nav.active()), Some(DiffRow::Gap { .. })) {
            return;
        }
        self.context += 3;
        let old_active = self.nav.active();
        self.rebuild();
        self.nav.set_active(old_active, self.rows.len());
    }



    fn render_side(side: &Side, col_width: usize, text_style: Style, no_style: Style) -> Vec<Span> {
        match side {
            Side::Line { no, text } => {
                let no_str = format!(" {:>3} ", no);
                let avail = col_width.saturating_sub(no_str.len());
                let truncated = Self::truncate(text, avail);
                let padded = format!("{:<width$}", truncated, width = avail);
                vec![
                    Span::styled(no_str, no_style).no_wrap(),
                    Span::styled(padded, text_style).no_wrap(),
                ]
            }
            Side::Empty => {
                vec![Span::styled(" ".repeat(col_width), Style::default()).no_wrap()]
            }
        }
    }

    fn truncate(s: &str, max: usize) -> String {
        s.chars().take(max).collect()
    }
}



impl Component for DiffOutput {
    fn children(&self) -> &[Node] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}



impl Drawable for DiffOutput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let total = self.rows.len();
        let (start, end) = self.nav.visible_range(total);

        let dim = Style::new().color(Color::DarkGrey);
        let no_st = Style::new().color(Color::Rgb(80, 80, 80));
        let ctx_st = Style::new().color(Color::Rgb(200, 200, 200));
        let add_st = Style::new()
            .color(Color::Green)
            .background(Color::Rgb(0, 35, 0));
        let del_st = Style::new()
            .color(Color::Red)
            .background(Color::Rgb(40, 0, 0));
        let chg_st = Style::new()
            .color(Color::Yellow)
            .background(Color::Rgb(38, 32, 0));
        let active_bg = Style::new().background(Color::Rgb(45, 45, 65));
        let active_dim = Style::new()
            .color(Color::Rgb(120, 120, 140))
            .background(Color::Rgb(45, 45, 65));

        let col_width = 38usize;

        let mut lines: Vec<Vec<Span>> = Vec::new();


        if !self.base.label().is_empty() {
            let n_chunks = self
                .rows
                .iter()
                .filter(|r| matches!(r, DiffRow::Gap { .. }))
                .count()
                + 1;
            lines.push(vec![
                Span::styled(format!("─── {} ", self.base.label()), dim).no_wrap(),
                Span::styled(
                    format!(
                        "[{} chunk{}]",
                        n_chunks,
                        if n_chunks == 1 { "" } else { "s" }
                    ),
                    dim,
                )
                .no_wrap(),
            ]);
        }

        for vis in start..end {
            let is_active = focused && vis == self.nav.active();

            match &self.rows[vis] {
                DiffRow::Gap { hidden } => {
                    let st = if is_active {
                        Style::new()
                            .color(Color::Cyan)
                            .background(Color::Rgb(45, 45, 65))
                    } else {
                        dim
                    };
                    let label = if is_active {
                        format!(" {} lines hidden [Enter: +3] ", hidden)
                    } else {
                        format!(" {} lines hidden [+3] ", hidden)
                    };
                    let fill = "┄".repeat(12);
                    lines.push(vec![
                        Span::styled(fill.clone(), st).no_wrap(),
                        Span::styled(label, Style::new().color(Color::Cyan)).no_wrap(),
                        Span::styled(fill, st).no_wrap(),
                    ]);
                }
                DiffRow::Line { left, right, kind } => {
                    let (marker, l_st, r_st) = match kind {
                        RowKind::Context => (
                            " ",
                            if is_active { active_bg } else { ctx_st },
                            if is_active { active_bg } else { ctx_st },
                        ),
                        RowKind::Removed => (
                            "-",
                            if is_active { active_bg } else { del_st },
                            Style::default(),
                        ),
                        RowKind::Added => (
                            "+",
                            Style::default(),
                            if is_active { active_bg } else { add_st },
                        ),
                        RowKind::Changed => (
                            "~",
                            if is_active { active_bg } else { chg_st },
                            if is_active { active_bg } else { chg_st },
                        ),
                    };

                    let marker_st = if is_active {
                        Style::new()
                            .color(Color::Yellow)
                            .background(Color::Rgb(45, 45, 65))
                    } else {
                        match kind {
                            RowKind::Removed => Style::new().color(Color::Red),
                            RowKind::Added => Style::new().color(Color::Green),
                            RowKind::Changed => Style::new().color(Color::Yellow),
                            RowKind::Context => dim,
                        }
                    };
                    let cursor_st = if is_active {
                        Style::new()
                            .color(Color::Yellow)
                            .background(Color::Rgb(45, 45, 65))
                    } else {
                        dim
                    };
                    let sep_st = if is_active { active_dim } else { dim };
                    let no_style = if is_active { active_dim } else { no_st };

                    let mut line = vec![
                        Span::styled(if is_active { "❯" } else { " " }, cursor_st).no_wrap(),
                        Span::styled(format!("{} ", marker), marker_st).no_wrap(),
                    ];
                    line.extend(Self::render_side(left, col_width, l_st, no_style));
                    line.push(Span::styled(" │ ", sep_st).no_wrap());
                    line.extend(Self::render_side(right, col_width, r_st, no_style));
                    lines.push(line);
                }
            }
        }

        if let Some(text) = self.nav.footer(total) {
            lines.push(vec![Span::styled(text, dim).no_wrap()]);
        }

        if focused {
            lines.push(vec![
                Span::styled(
                    "  ↑↓ navigate  Tab next chunk  Shift+Tab prev  Enter expand gap",
                    dim,
                )
                .no_wrap(),
            ]);
        }

        DrawOutput { lines }
    }
}



impl Interactive for DiffOutput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Up => {
                self.move_cursor(-1);
                InteractionResult::handled()
            }
            KeyCode::Down => {
                self.move_cursor(1);
                InteractionResult::handled()
            }
            KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                self.next_chunk();
                InteractionResult::handled()
            }
            KeyCode::BackTab => {
                self.prev_chunk();
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                self.expand_gap();
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<crate::core::value::Value> {
        None
    }
    fn set_value(&mut self, _: crate::core::value::Value) {}
    fn validate(&self, _: ValidationMode) -> Result<(), String> {
        Ok(())
    }
    fn cursor_pos(&self) -> Option<crate::terminal::CursorPos> {
        None
    }
}
