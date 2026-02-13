use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use crate::widgets::validators::Validator;

pub struct CheckboxInput {
    base: InputBase,
    checked: bool,
    validators: Vec<Validator>,
}

impl CheckboxInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            checked: false,
            validators: Vec::new(),
        }
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn value_text(&self) -> &'static str {
        if self.checked { "true" } else { "false" }
    }

    fn set_from_text(&mut self, text: &str) {
        self.checked = matches!(text.to_ascii_lowercase().as_str(), "true" | "1" | "yes");
    }
}

impl Drawable for CheckboxInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line = self.base.line_state(ctx);

        let (value_text, value_style) = if self.checked {
            ("✓".to_string(), Style::new().color(Color::Green))
        } else {
            ("✗".to_string(), Style::new().color(Color::Red))
        };

        DrawOutput {
            lines: vec![vec![
                Span::new(line.prefix).no_wrap(),
                Span::styled(value_text, value_style).no_wrap(),
            ]],
        }
    }
}

impl Interactive for CheckboxInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(' ') => {
                self.checked = !self.checked;
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::submit_requested(),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Bool(self.checked))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(flag) = value.to_bool() {
            self.checked = flag;
        } else if let Some(text) = value.as_text() {
            self.set_from_text(text);
        }
    }

    fn validate_submit(&self) -> Result<(), String> {
        for validator in &self.validators {
            validator(self.value_text())?;
        }
        Ok(())
    }
}
