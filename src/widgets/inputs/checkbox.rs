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

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
        self
    }
}

impl Drawable for CheckboxInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let span = if focused {
            let (symbol, style) = if self.checked {
                ("[✓]", Style::new().color(Color::Green))
            } else {
                ("[✗]", Style::new().color(Color::Red))
            };
            Span::styled(symbol, style).no_wrap()
        } else {
            let (text, style) = if self.checked {
                ("true", Style::new().color(Color::Green))
            } else {
                ("false", Style::new().color(Color::Red))
            };
            Span::styled(text, style).no_wrap()
        };

        DrawOutput {
            lines: vec![vec![span]],
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
            KeyCode::Enter => InteractionResult::input_done(),
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
        run_validators(&self.validators, &Value::Bool(self.checked))
    }
}
