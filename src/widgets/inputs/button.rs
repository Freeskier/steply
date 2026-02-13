use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::task::{TaskId, TaskRequest};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use crate::widgets::validators::Validator;

pub struct ButtonInput {
    base: InputBase,
    text: String,
    clicks: i64,
    validators: Vec<Validator>,
    task_request: Option<TaskRequest>,
}

impl ButtonInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            base: InputBase::new(id, label.clone()),
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

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let line = self.base.line_state(ctx);

        let label = if self.text.is_empty() {
            " ".to_string()
        } else {
            self.text.clone()
        };

        let value_style = if line.focused {
            Style::new()
                .color(Color::White)
                .background(Color::Blue)
                .bold()
        } else {
            Style::new().color(Color::DarkGrey)
        };

        DrawOutput {
            lines: vec![vec![
                Span::new(line.prefix).no_wrap(),
                Span::styled(label, value_style).no_wrap(),
            ]],
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
                if let Some(task_request) = self.task_request.clone() {
                    return InteractionResult::with_event(WidgetEvent::TaskRequested {
                        request: task_request,
                    });
                }
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Float(self.clicks as f64))
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::Float(number) => self.clicks = number.round() as i64,
            Value::Text(text) => self.clicks = text.parse::<f64>().map_or(0, |v| v.round() as i64),
            _ => {}
        }
    }

    fn validate_submit(&self) -> Result<(), String> {
        let text_value = self.clicks.to_string();
        for validator in &self.validators {
            validator(text_value.as_str())?;
        }
        Ok(())
    }
}
