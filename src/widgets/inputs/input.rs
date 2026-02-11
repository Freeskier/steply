use super::text_edit;
use super::validators::Validator;
use crate::app::event::WidgetEvent;
use crate::domain::value::Value;
use crate::terminal::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, TextAction,
};

pub struct Input {
    base: InputBase,
    value: String,
    cursor: usize,
    submit_target: Option<String>,
    validators: Vec<Validator>,
}

impl Input {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            value: String::new(),
            cursor: 0,
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
}

impl Drawable for Input {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let prefix = format!("{} {}: ", self.base.focus_marker(), self.base.label());

        let (value_text, value_style) = if let Some(error) = ctx.visible_errors.get(self.base.id())
        {
            (
                format!("✗ {}", error),
                Style::new().color(Color::Red).bold(),
            )
        } else if ctx.invalid_hidden.contains(self.base.id()) {
            (self.value.clone(), Style::new().color(Color::Red))
        } else {
            (self.value.clone(), Style::default())
        };

        DrawOutput {
            lines: vec![vec![
                Span::new(prefix).no_wrap(),
                Span::styled(value_text, value_style).no_wrap(),
            ]],
        }
    }
}

impl Interactive for Input {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn is_focused(&self) -> bool {
        self.base.is_focused()
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.set_focused(focused);
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(ch) => {
                text_edit::insert_char(&mut self.value, &mut self.cursor, ch);
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if text_edit::backspace_char(&mut self.value, &mut self.cursor) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Left => {
                if text_edit::move_left(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if text_edit::move_right(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter => {
                if let Some(target) = &self.submit_target {
                    return InteractionResult::with_event(WidgetEvent::ValueProduced {
                        target: target.clone(),
                        value: Value::Text(self.value.clone()),
                    });
                }
                InteractionResult::with_event(WidgetEvent::RequestSubmit)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let handled = match action {
            TextAction::DeleteWordLeft => {
                text_edit::delete_word_left(&mut self.value, &mut self.cursor)
            }
            TextAction::DeleteWordRight => {
                text_edit::delete_word_right(&mut self.value, &mut self.cursor)
            }
        };
        if handled {
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueProduced { target, value } if target == self.base.id() => {
                if let Value::Text(v) = value {
                    self.value = v.clone();
                    self.cursor = text_edit::char_count(&self.value);
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.value.clone()))
    }

    fn set_value(&mut self, value: Value) {
        if let Value::Text(v) = value {
            self.value = v;
            self.cursor = text_edit::char_count(&self.value);
        }
    }

    fn validate(&self) -> Result<(), String> {
        for validator in &self.validators {
            validator(&self.value)?;
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.base.is_focused() {
            Some(CursorPos {
                col: (self.base.label().len() + self.cursor + 4) as u16,
                row: 0,
            })
        } else {
            None
        }
    }
}
