use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::text_edit;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem, InteractionResult,
    Interactive, RenderContext, TextAction, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use unicode_width::UnicodeWidthChar;

pub struct TextAreaComponent {
    id: String,

    lines: Vec<String>,

    row: usize,

    col: usize,
    scroll: ScrollState,
    min_height: usize,
    max_height: usize,
    validators: Vec<Validator>,
}

impl TextAreaComponent {
    pub fn new(id: impl Into<String>) -> Self {
        let max_height = 8;
        Self {
            id: id.into(),
            lines: vec![String::new()],
            row: 0,
            col: 0,
            scroll: ScrollState::new(Some(max_height)),
            min_height: 3,
            max_height,
            validators: Vec::new(),
        }
    }

    pub fn with_min_height(mut self, n: usize) -> Self {
        self.min_height = n.max(1);
        self
    }

    pub fn with_max_height(mut self, n: usize) -> Self {
        self.max_height = n.max(1);
        self.scroll = ScrollState::new(Some(self.max_height));
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
        self
    }

    pub fn with_validator(mut self, v: Validator) -> Self {
        self.validators.push(v);
        self
    }

    fn num_width(&self) -> usize {
        self.lines.len().to_string().len()
    }

    fn gutter_width(&self) -> usize {
        1 + 1 + self.num_width() + 2
    }

    fn visible_height(&self) -> usize {
        self.lines.len().clamp(self.min_height, self.max_height)
    }

    fn split_line(&mut self) {
        let col = self.col.min(text_edit::char_count(&self.lines[self.row]));
        let line = &self.lines[self.row];
        let byte = text_edit::byte_index_at_char(line, col);
        let right = line[byte..].to_string();
        self.lines[self.row].truncate(byte);
        self.row += 1;
        self.col = 0;
        self.lines.insert(self.row, right);
        self.scroll.ensure_visible(self.row, self.lines.len());
    }

    fn merge_with_prev(&mut self) {
        if self.row == 0 {
            return;
        }
        let prev_len = text_edit::char_count(&self.lines[self.row - 1]);
        let current = self.lines.remove(self.row);
        self.lines[self.row - 1].push_str(&current);
        self.row -= 1;
        self.col = prev_len;
        self.scroll.ensure_visible(self.row, self.lines.len());
    }

    fn merge_with_next(&mut self) {
        if self.row + 1 >= self.lines.len() {
            return;
        }
        let next = self.lines.remove(self.row + 1);
        self.lines[self.row].push_str(&next);
        self.scroll.ensure_visible(self.row, self.lines.len());
    }

    fn current_line_len(&self) -> usize {
        text_edit::char_count(&self.lines[self.row])
    }

    fn build_gutter_span(&self, line_idx: usize, _focused: bool) -> Span {
        let num_w = self.num_width();
        let num_str = format!("{:>width$}", line_idx + 1, width = num_w);
        let text = format!("┃ {}  ", num_str);
        Span::styled(text, Style::new().color(Color::DarkGrey).no_strikethrough()).no_wrap()
    }

    fn build_tilde_span(&self) -> Span {
        let num_w = self.num_width();
        let pad = num_w + 1;
        let text = format!("┃ ~{:pad$}", "", pad = pad);
        Span::styled(text, Style::new().color(Color::DarkGrey).no_strikethrough()).no_wrap()
    }

    fn line_display_col(&self, row: usize, col: usize) -> usize {
        self.lines[row]
            .chars()
            .take(col.min(text_edit::char_count(&self.lines[row])))
            .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(1))
            .sum()
    }
}

impl Drawable for TextAreaComponent {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        ""
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx.focused_id.as_deref().is_some_and(|id| id == self.id);
        let visible = self.visible_height();
        let (start, _) = self.scroll.visible_range(self.lines.len());

        let mut output_lines = Vec::with_capacity(visible + 1);

        for i in 0..visible {
            let real_idx = start + i;
            if real_idx < self.lines.len() {
                let gutter = self.build_gutter_span(real_idx, focused);
                let content = Span::new(self.lines[real_idx].clone()).no_wrap();
                output_lines.push(vec![gutter, content]);
            } else {
                output_lines.push(vec![self.build_tilde_span()]);
            }
        }

        if let Some(footer) = self.scroll.footer(self.lines.len()) {
            output_lines.push(vec![
                Span::styled(
                    format!("  {}", footer),
                    Style::new().color(Color::DarkGrey).no_strikethrough(),
                )
                .no_wrap(),
            ]);
        }

        DrawOutput::with_lines(output_lines)
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }
        vec![
            HintItem::new("Shift+Enter / Alt+Enter", "new line", HintGroup::Edit).with_priority(10),
            HintItem::new("Enter / Esc", "finish", HintGroup::Action).with_priority(20),
            HintItem::new("← → ↑ ↓", "move cursor", HintGroup::Navigation).with_priority(11),
            HintItem::new("Home / End", "line start/end", HintGroup::Navigation).with_priority(12),
        ]
    }
}

impl LeafComponent for TextAreaComponent {}

impl Interactive for TextAreaComponent {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => InteractionResult::input_done(),
            KeyCode::Enter
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.split_line();
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::input_done(),
            KeyCode::Char(_)
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => {
                let row = self.row;
                let outcome =
                    text_edit::apply_single_line_key(&mut self.lines[row], &mut self.col, key);
                match outcome {
                    text_edit::TextKeyOutcome::Ignored => InteractionResult::ignored(),
                    text_edit::TextKeyOutcome::Changed | text_edit::TextKeyOutcome::CursorMoved => {
                        self.scroll.ensure_visible(self.row, self.lines.len());
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::BackspaceAtStart => {
                        self.merge_with_prev();
                        self.scroll.ensure_visible(self.row, self.lines.len());
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::DeleteAtEnd => {
                        self.merge_with_next();
                        self.scroll.ensure_visible(self.row, self.lines.len());
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::MoveLeftAtStart => {
                        if self.row > 0 {
                            self.row -= 1;
                            self.col = self.current_line_len();
                            self.scroll.ensure_visible(self.row, self.lines.len());
                        }
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::MoveRightAtEnd => {
                        if self.row + 1 < self.lines.len() {
                            self.row += 1;
                            self.col = 0;
                            self.scroll.ensure_visible(self.row, self.lines.len());
                        }
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::Submit => InteractionResult::ignored(),
                }
            }

            KeyCode::Up => {
                if self.row > 0 {
                    self.row -= 1;
                    self.col = self.col.min(self.current_line_len());
                    self.scroll.ensure_visible(self.row, self.lines.len());
                }
                InteractionResult::handled()
            }
            KeyCode::Down => {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = self.col.min(self.current_line_len());
                    self.scroll.ensure_visible(self.row, self.lines.len());
                }
                InteractionResult::handled()
            }

            _ => InteractionResult::ignored(),
        }
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        Some(TextEditState {
            value: &mut self.lines[self.row],
            cursor: &mut self.col,
        })
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if action == TextAction::DeleteWordLeft && self.col == 0 && self.row > 0 {
            self.merge_with_prev();
            return InteractionResult::handled();
        }

        if action == TextAction::MoveWordLeft && self.col == 0 && self.row > 0 {
            self.row -= 1;
            self.col = text_edit::char_count(&self.lines[self.row]);
            self.scroll.ensure_visible(self.row, self.lines.len());
            return InteractionResult::handled();
        }

        if action == TextAction::MoveWordRight
            && self.col >= text_edit::char_count(&self.lines[self.row])
            && self.row + 1 < self.lines.len()
        {
            self.row += 1;
            self.col = 0;
            self.scroll.ensure_visible(self.row, self.lines.len());
            return InteractionResult::handled();
        }

        let Some(mut state) = self.text_editing() else {
            return InteractionResult::ignored();
        };
        if action.apply(&mut state) {
            return InteractionResult::handled();
        }
        InteractionResult::ignored()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.lines.join("\n")))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.as_text() {
            self.lines = text.split('\n').map(String::from).collect();
            if self.lines.is_empty() {
                self.lines = vec![String::new()];
            }
            self.row = 0;
            self.col = 0;
            self.scroll = ScrollState::new(Some(self.max_height));
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &Value::Text(self.lines.join("\n")))
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let (start, _) = self.scroll.visible_range(self.lines.len());
        let visible_row = self.row.saturating_sub(start);
        let display_col = self.line_display_col(self.row, self.col);
        let col = self.gutter_width() + display_col;
        Some(CursorPos {
            row: visible_row as u16,
            col: col as u16,
        })
    }
}
