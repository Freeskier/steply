use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::Style;
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::Validator;

pub struct SelectInput {
    base: WidgetBase,
    options: Vec<String>,
    selected: usize,
    submit_target: Option<String>,
    validators: Vec<Validator>,
}

impl SelectInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            options,
            selected: 0,
            submit_target: None,
            validators: Vec::new(),
        }
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self.clamp_selected();
        self
    }

    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options;
        self.clamp_selected();
    }

    fn clamp_selected(&mut self) {
        if self.options.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.options.len() {
            self.selected = self.options.len() - 1;
        }
    }

    fn selected_text(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn move_left(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let len = self.options.len();
        self.selected = (self.selected + len - 1) % len;
        true
    }

    fn move_right(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let len = self.options.len();
        self.selected = (self.selected + 1) % len;
        true
    }
}

impl Drawable for SelectInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let prefix = self.base.input_prefix(ctx);
        DrawOutput {
            lines: vec![vec![
                Span::new(prefix).no_wrap(),
                Span::styled(format!("‹ {} ›", self.selected_text()), Style::default()).no_wrap(),
            ]],
        }
    }
}

impl Interactive for SelectInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Left => {
                if self.move_left() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if self.move_right() {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
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
            && let Some(position) = self.options.iter().position(|option| option == &text)
        {
            self.selected = position;
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        use crate::widgets::validators::run_validators;
        run_validators(&self.validators, self.selected_text())
    }
}
