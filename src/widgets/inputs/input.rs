use crate::app::event::WidgetEvent;
use crate::domain::value::Value;
use crate::terminal::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::widgets::base::InputBase;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};

pub struct Input {
    base: InputBase,
    value: String,
    cursor: usize,
    submit_target: Option<String>,
}

impl Input {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: InputBase::new(id, label),
            value: String::new(),
            cursor: 0,
            submit_target: None,
        }
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }
}

impl Drawable for Input {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput::plain_lines(vec![format!(
            "{} {}: {}",
            self.base.focus_marker(),
            self.base.label(),
            self.value
        )])
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
                self.value.insert(self.cursor, ch);
                self.cursor += 1;
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
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

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueProduced { target, value } if target == self.base.id() => {
                if let Value::Text(v) = value {
                    self.value = v.clone();
                    self.cursor = self.value.len();
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
            self.cursor = self.value.len();
        }
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
