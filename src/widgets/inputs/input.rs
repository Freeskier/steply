use super::text_edit;
use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextEditState,
};
use crate::widgets::validators::Validator;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct Input {
    base: InputBase,
    value: String,
    cursor: usize,
    submit_target: Option<String>,
    validators: Vec<Validator>,
    completion_items: Vec<String>,
}

impl Input {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            value: String::new(),
            cursor: 0,
            submit_target: None,
            validators: Vec::new(),
            completion_items: Vec::new(),
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

    pub fn with_completion_items(mut self, items: Vec<String>) -> Self {
        self.completion_items = items;
        self
    }

    pub fn set_completion_items(&mut self, items: Vec<String>) {
        self.completion_items = items;
    }

    pub fn completion_items_mut(&mut self) -> &mut Vec<String> {
        &mut self.completion_items
    }
}

impl Drawable for Input {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let is_focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());
        let prefix = format!(
            "{} {}: ",
            self.base.focus_marker(is_focused),
            self.base.label()
        );

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
                        target: target.as_str().into(),
                        value: Value::Text(self.value.clone()),
                    });
                }
                InteractionResult::with_event(WidgetEvent::RequestSubmit)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn text_edit_state(&mut self) -> Option<TextEditState<'_>> {
        Some(TextEditState {
            value: &mut self.value,
            cursor: &mut self.cursor,
        })
    }

    fn completion_state(&mut self) -> Option<CompletionState<'_>> {
        Some(CompletionState {
            value: &mut self.value,
            cursor: &mut self.cursor,
            items: &mut self.completion_items,
        })
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueProduced { target, value } if target.as_str() == self.base.id() => {
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
        let prefix = format!("{} {}: ", self.base.focus_marker(true), self.base.label());
        let mut value_width = 0usize;
        for ch in self
            .value
            .chars()
            .take(text_edit::clamp_cursor(self.cursor, &self.value))
        {
            value_width = value_width.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        }
        Some(CursorPos {
            col: (UnicodeWidthStr::width(prefix.as_str()) + value_width) as u16,
            row: 0,
        })
    }
}
