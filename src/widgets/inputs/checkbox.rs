use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct CheckboxInput {
    base: WidgetBase,
    checked: bool,
    validators: Vec<Validator>,
}

impl CheckboxInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
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

    fn value_str(&self) -> &'static str {
        if self.checked { "true" } else { "false" }
    }
}

impl Drawable for CheckboxInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let (symbol, style) = if self.checked {
            ("✓", Style::new().color(Color::Green))
        } else {
            ("✗", Style::new().color(Color::Red))
        };

        DrawOutput {
            lines: vec![vec![
                Span::new(self.base.input_prefix(ctx)).no_wrap(),
                Span::styled(symbol, style).no_wrap(),
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
            self.checked = matches!(text.to_ascii_lowercase().as_str(), "true" | "1" | "yes");
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, self.value_str())
    }
}
