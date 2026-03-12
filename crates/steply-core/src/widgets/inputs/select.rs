use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::inline::{Inline, InlineGroup};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::shared::list_nav;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct SelectInput {
    base: WidgetBase,
    options: Vec<String>,
    selected: usize,
    validators: Vec<Validator>,
}

impl SelectInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            options,
            selected: 0,
            validators: Vec::new(),
        }
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
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
}

impl Drawable for SelectInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        if self.base.is_focused(ctx) {
            let control = Inline::group(InlineGroup::no_break(vec![
                Inline::text(Span::styled("‹", Style::new().color(Color::Yellow).bold())),
                Inline::text(Span::new(" ")),
                Inline::text(Span::styled(
                    self.selected_text().to_string(),
                    Style::default(),
                )),
                Inline::text(Span::new(" ")),
                Inline::text(Span::styled("›", Style::new().color(Color::Yellow).bold())),
            ]));
            return DrawOutput::with_inline_lines(vec![vec![control]]);
        }

        DrawOutput::with_lines(vec![vec![
            Span::styled(self.selected_text().to_string(), Style::default()).no_wrap(),
        ]])
    }
}

impl Interactive for SelectInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Left => InteractionResult::handled_if(list_nav::apply_cycle_index(
                &mut self.selected,
                self.options.len(),
                true,
            )),
            KeyCode::Right => InteractionResult::handled_if(list_nav::apply_cycle_index(
                &mut self.selected,
                self.options.len(),
                false,
            )),
            KeyCode::Enter => InteractionResult::input_done(),
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

    fn set_options_from_value(&mut self, value: Value) -> bool {
        let Some(options) = value.as_list().map(|items| {
            items
                .iter()
                .filter_map(Value::to_text_scalar)
                .collect::<Vec<_>>()
        }) else {
            return false;
        };
        if self.options == options {
            return false;
        }
        self.set_options(options);
        true
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(
            &self.validators,
            &Value::Text(self.selected_text().to_string()),
        )
    }
}
