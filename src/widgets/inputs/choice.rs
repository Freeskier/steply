use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct ChoiceInput {
    base: WidgetBase,
    options: Vec<String>,
    selected: usize,
    show_bullets: bool,
    submit_target: Option<String>,
    validators: Vec<Validator>,
}

impl ChoiceInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            options,
            selected: 0,
            show_bullets: true,
            submit_target: None,
            validators: Vec::new(),
        }
    }

    pub fn with_bullets(mut self, enabled: bool) -> Self {
        self.show_bullets = enabled;
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn selected_text(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn move_prev(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let len = self.options.len();
        self.selected = (self.selected + len - 1) % len;
        true
    }

    fn move_next(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }
        self.selected = (self.selected + 1) % self.options.len();
        true
    }

    fn select_by_letter(&mut self, ch: char) -> bool {
        let needle = ch.to_ascii_lowercase();
        if let Some(index) = self.options.iter().position(|opt| {
            opt.chars()
                .next()
                .map(|c| c.to_ascii_lowercase() == needle)
                .unwrap_or(false)
        }) {
            self.selected = index;
            return true;
        }
        false
    }
}

impl Drawable for ChoiceInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let active_style = Style::new().color(Color::Cyan).bold();
        let inactive_style = Style::new().color(Color::DarkGrey);

        let mut spans = vec![];
        for (index, option) in self.options.iter().enumerate() {
            if index > 0 {
                spans.push(Span::new(" / ").no_wrap());
            }
            if self.show_bullets {
                if index == self.selected {
                    spans
                        .push(Span::styled("●", Style::new().color(Color::Green).bold()).no_wrap());
                } else {
                    spans.push(Span::styled("○", inactive_style).no_wrap());
                }
                spans.push(Span::new(" ").no_wrap());
            }
            let style = if index == self.selected {
                active_style
            } else {
                inactive_style
            };
            spans.push(Span::styled(option.clone(), style).no_wrap());
        }

        DrawOutput { lines: vec![spans] }
    }
}

impl Interactive for ChoiceInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Left | KeyCode::Up => {
                if self.move_prev() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Right | KeyCode::Down => {
                if self.move_next() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Char(ch) => {
                if self.select_by_letter(ch) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Enter => InteractionResult::submit_or_produce(
                self.submit_target.as_deref(),
                Value::Text(self.selected_text().to_string()),
            ),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.selected_text().to_string()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar()
            && let Some(pos) = self.options.iter().position(|opt| opt == &text)
        {
            self.selected = pos;
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, self.selected_text())
    }
}
