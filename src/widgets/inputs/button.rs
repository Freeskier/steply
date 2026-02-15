use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::task::{TaskId, TaskRequest};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct ButtonInput {
    base: WidgetBase,
    text: String,
    clicks: i64,
    validators: Vec<Validator>,
    task_request: Option<TaskRequest>,
}

impl ButtonInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            base: WidgetBase::new(id, label.clone()),
            text: label,
            clicks: 0,
            validators: Vec::new(),
            task_request: None,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_task_request(mut self, task_request: TaskRequest) -> Self {
        self.task_request = Some(task_request);
        self
    }

    pub fn with_task_id(mut self, task_id: impl Into<TaskId>) -> Self {
        self.task_request = Some(TaskRequest::new(task_id));
        self
    }
}

impl Drawable for ButtonInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let label = if self.text.is_empty() {
            " "
        } else {
            &self.text
        };
        let style = if focused {
            Style::new()
                .color(Color::White)
                .background(Color::Blue)
                .bold()
        } else {
            Style::new().color(Color::DarkGrey)
        };

        DrawOutput {
            lines: vec![vec![Span::styled(label, style).no_wrap()]],
        }
    }
}

impl Interactive for ButtonInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.clicks = self.clicks.saturating_add(1);
                if let Some(request) = self.task_request.clone() {
                    return InteractionResult::with_event(WidgetEvent::TaskRequested { request });
                }
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Number(self.clicks as f64))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(number) = value.to_number() {
            self.clicks = number.round() as i64;
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &self.clicks.to_string())
    }
}
