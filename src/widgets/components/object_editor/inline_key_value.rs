use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::traits::{InteractionResult, Interactive, TextAction};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InlineKeyValueFocus {
    #[default]
    Key,
    Value,
}

enum InlineValueField {
    Select(SelectInput),
    Text(TextInput),
}

pub struct InlineKeyValueEditor {
    key_input: TextInput,
    value_field: InlineValueField,
    focus: InlineKeyValueFocus,
}

impl InlineKeyValueEditor {
    pub fn new(
        id: impl Into<String>,
        _label: impl Into<String>,
        value_options: Vec<String>,
    ) -> Self {
        let id = id.into();
        Self {
            key_input: TextInput::new(format!("{id}__key"), ""),
            value_field: InlineValueField::Select(SelectInput::new(
                format!("{id}__value_type"),
                "",
                value_options,
            )),
            focus: InlineKeyValueFocus::Key,
        }
    }

    pub fn new_text(id: impl Into<String>, _label: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            key_input: TextInput::new(format!("{id}__key"), ""),
            value_field: InlineValueField::Text(TextInput::new(format!("{id}__value"), "")),
            focus: InlineKeyValueFocus::Key,
        }
    }

    pub fn with_default_key(mut self, key: impl Into<String>) -> Self {
        self.key_input.set_value(Value::Text(key.into()));
        self
    }

    pub fn with_default_value(mut self, value: impl Into<String>) -> Self {
        let value = Value::Text(value.into());
        match &mut self.value_field {
            InlineValueField::Select(select) => select.set_value(value),
            InlineValueField::Text(text) => text.set_value(value),
        }
        self
    }

    pub fn key(&self) -> String {
        self.key_input
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    pub fn value_type(&self) -> String {
        self.value_text()
    }

    pub fn value_text(&self) -> String {
        match &self.value_field {
            InlineValueField::Select(select) => select
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
            InlineValueField::Text(text) => text
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
        }
    }

    pub fn focus(&self) -> InlineKeyValueFocus {
        self.focus
    }

    pub fn set_focus(&mut self, focus: InlineKeyValueFocus) {
        self.focus = focus;
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    InlineKeyValueFocus::Key => InlineKeyValueFocus::Value,
                    InlineKeyValueFocus::Value => InlineKeyValueFocus::Key,
                };
            }
            _ => match self.focus {
                InlineKeyValueFocus::Key => {
                    self.key_input.on_key(key);
                }
                InlineKeyValueFocus::Value => match &mut self.value_field {
                    InlineValueField::Select(select) => {
                        if matches!(key.code, KeyCode::Left | KeyCode::Right) {
                            select.on_key(key);
                        }
                    }
                    InlineValueField::Text(text) => {
                        text.on_key(key);
                    }
                },
            },
        }
    }

    pub fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        match self.focus {
            InlineKeyValueFocus::Key => self.key_input.on_text_action(action),
            InlineKeyValueFocus::Value => match &mut self.value_field {
                InlineValueField::Select(_) => InteractionResult::ignored(),
                InlineValueField::Text(text) => text.on_text_action(action),
            },
        }
    }

    pub fn inline_spans(&self) -> Vec<Span> {
        let key = self.key();
        let value = self.value_text();
        let active = Style::new().color(Color::Cyan);
        let inactive = Style::new().color(Color::DarkGrey);
        let key_style = if self.focus == InlineKeyValueFocus::Key {
            active
        } else {
            inactive
        };
        let value_style = if self.focus == InlineKeyValueFocus::Value {
            active
        } else {
            inactive
        };
        let value_span = match &self.value_field {
            InlineValueField::Select(_) => {
                Span::styled(format!("‹ {value} ›"), value_style).no_wrap()
            }
            InlineValueField::Text(_) => Span::styled(value, value_style).no_wrap(),
        };
        vec![
            Span::styled(key, key_style).no_wrap(),
            Span::new(": ").no_wrap(),
            value_span,
        ]
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        match self.focus {
            InlineKeyValueFocus::Key => self.key_input.cursor_pos(),
            InlineKeyValueFocus::Value => match &self.value_field {
                InlineValueField::Select(_) => None,
                InlineValueField::Text(text) => {
                    let key_width: u16 = self
                        .key()
                        .chars()
                        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0) as u16)
                        .sum();
                    let cursor = text.cursor_pos()?;
                    Some(CursorPos {
                        col: key_width.saturating_add(2).saturating_add(cursor.col),
                        row: 0,
                    })
                }
            },
        }
    }
}
