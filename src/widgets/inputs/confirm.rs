use super::text_edit;
use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};


#[derive(Debug, Clone)]
pub enum ConfirmMode {

    Relaxed,

    Strict { word: String },
}

impl Default for ConfirmMode {
    fn default() -> Self {
        Self::Relaxed
    }
}

pub struct ConfirmInput {
    base: WidgetBase,

    yes_label: String,

    no_label: String,

    confirmed: Option<bool>,
    mode: ConfirmMode,

    buffer: String,
    cursor: usize,

    strict_error: bool,
}

impl ConfirmInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            yes_label: "yes".into(),
            no_label: "no".into(),
            confirmed: None,
            mode: ConfirmMode::Relaxed,
            buffer: String::new(),
            cursor: 0,
            strict_error: false,
        }
    }

    pub fn with_mode(mut self, mode: ConfirmMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_options(
        mut self,
        yes_label: impl Into<String>,
        no_label: impl Into<String>,
    ) -> Self {
        self.yes_label = yes_label.into();
        self.no_label = no_label.into();
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
        self
    }

    fn confirm(&mut self) {
        self.confirmed = Some(true);
        self.strict_error = false;
    }

    fn decline(&mut self) {
        self.confirmed = Some(false);
        self.strict_error = false;
        self.buffer.clear();
        self.cursor = 0;
    }
}

impl Drawable for ConfirmInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);

        let spans = if !focused {

            match self.confirmed {
                Some(true) => vec![
                    Span::styled(self.yes_label.clone(), Style::new().color(Color::Green))
                        .no_wrap(),
                ],
                Some(false) => vec![
                    Span::styled(self.no_label.clone(), Style::new().color(Color::Red)).no_wrap(),
                ],
                None => vec![
                    Span::new(self.yes_label.clone()).no_wrap(),
                    Span::styled(" / ", Style::new().color(Color::DarkGrey)).no_wrap(),
                    Span::new(self.no_label.clone()).no_wrap(),
                ],
            }
        } else {
            match &self.mode {
                ConfirmMode::Relaxed => {

                    vec![
                        Span::styled(
                            format!("[{}]", self.yes_label),
                            Style::new().color(Color::Green).bold(),
                        )
                        .no_wrap(),
                        Span::styled(" / ", Style::new().color(Color::DarkGrey)).no_wrap(),
                        Span::styled(self.no_label.clone(), Style::new().color(Color::DarkGrey))
                            .no_wrap(),
                    ]
                }
                ConfirmMode::Strict { word } => {

                    let prompt = format!("Type \"{}\" to confirm: ", word);
                    let mut s = vec![
                        Span::styled(prompt, Style::new().color(Color::DarkGrey)).no_wrap(),
                        Span::new(self.buffer.clone()).no_wrap(),
                    ];
                    if self.strict_error {
                        s.push(
                            Span::styled(
                                format!("  ✗ Type \"{}\" to confirm", word),
                                Style::new().color(Color::Red),
                            )
                            .no_wrap(),
                        );
                    }
                    s
                }
            }
        };

        DrawOutput { lines: vec![spans] }
    }
}

impl Interactive for ConfirmInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match &self.mode.clone() {
            ConfirmMode::Relaxed => match key.code {
                KeyCode::Enter => {
                    if self.confirmed.is_none() {
                        self.confirm();
                    }
                    InteractionResult::input_done()
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.confirm();
                    InteractionResult::input_done()
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.decline();
                    InteractionResult::input_done()
                }
                _ => InteractionResult::ignored(),
            },
            ConfirmMode::Strict { word } => {
                let word = word.clone();
                match key.code {
                    KeyCode::Enter => {
                        if self.buffer.to_ascii_lowercase() == word.to_ascii_lowercase() {
                            self.confirm();
                            InteractionResult::input_done()
                        } else {
                            self.strict_error = true;
                            InteractionResult::handled()
                        }
                    }
                    KeyCode::Char(ch) => {
                        text_edit::insert_char(&mut self.buffer, &mut self.cursor, ch);
                        self.strict_error = false;
                        InteractionResult::handled()
                    }
                    KeyCode::Backspace => {
                        if text_edit::backspace_char(&mut self.buffer, &mut self.cursor) {
                            self.strict_error = false;
                            return InteractionResult::handled();
                        }
                        InteractionResult::ignored()
                    }
                    KeyCode::Left => {
                        text_edit::move_left(&mut self.cursor, &self.buffer);
                        InteractionResult::handled()
                    }
                    KeyCode::Right => {
                        text_edit::move_right(&mut self.cursor, &self.buffer);
                        InteractionResult::handled()
                    }
                    _ => InteractionResult::ignored(),
                }
            }
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Bool(self.confirmed.unwrap_or(false)))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(flag) = value.to_bool() {
            self.confirmed = Some(flag);
        } else if let Some(text) = value.as_text() {
            self.confirmed = Some(matches!(
                text.to_ascii_lowercase().as_str(),
                "true" | "1" | "yes"
            ));
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if let ConfirmMode::Strict { word } = &self.mode {

            let prompt_len = format!("Type \"{}\" to confirm: ", word).chars().count();
            let cursor_chars = self.cursor.min(text_edit::char_count(&self.buffer));
            let col = prompt_len + cursor_chars;
            Some(CursorPos {
                row: 0,
                col: col as u16,
            })
        } else {
            None
        }
    }
}
