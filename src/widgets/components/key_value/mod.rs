use indexmap::IndexMap;

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextEditState, ValidationMode,
};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyValueFocus {
    #[default]
    Key,
    Value,
}

pub struct KeyValueComponent {
    base: WidgetBase,
    key_input: TextInput,
    value_field: ValueField,
    focus: KeyValueFocus,
    submit_target: Option<ValueTarget>,
}

enum ValueField {
    Select(SelectInput),
    Text(TextInput),
}

impl KeyValueComponent {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        value_options: Vec<String>,
    ) -> Self {
        let id = id.into();
        Self {
            base: WidgetBase::new(id.clone(), label),
            key_input: TextInput::new(format!("{id}__key"), ""),
            value_field: ValueField::Select(SelectInput::new(
                format!("{id}__value_type"),
                "",
                value_options,
            )),
            focus: KeyValueFocus::Key,
            submit_target: None,
        }
    }

    pub fn new_text(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            base: WidgetBase::new(id.clone(), label),
            key_input: TextInput::new(format!("{id}__key"), ""),
            value_field: ValueField::Text(TextInput::new(format!("{id}__value"), "")),
            focus: KeyValueFocus::Key,
            submit_target: None,
        }
    }

    pub fn with_default_key(mut self, key: impl Into<String>) -> Self {
        self.key_input.set_value(Value::Text(key.into()));
        self
    }

    pub fn with_default_value_type(mut self, value_type: impl Into<String>) -> Self {
        if let ValueField::Select(select) = &mut self.value_field {
            select.set_value(Value::Text(value_type.into()));
        }
        self
    }

    pub fn with_default_value(mut self, value: impl Into<String>) -> Self {
        if let ValueField::Text(text) = &mut self.value_field {
            text.set_value(Value::Text(value.into()));
        }
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn key(&self) -> String {
        self.key_input
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    pub fn value_type(&self) -> String {
        match &self.value_field {
            ValueField::Select(select) => select
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
            ValueField::Text(text) => text
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
        }
    }

    pub fn value_text(&self) -> String {
        match &self.value_field {
            ValueField::Select(select) => select
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
            ValueField::Text(text) => text
                .value()
                .and_then(|value| value.to_text_scalar())
                .unwrap_or_default(),
        }
    }

    pub fn focus(&self) -> KeyValueFocus {
        self.focus
    }

    pub fn set_focus(&mut self, focus: KeyValueFocus) {
        self.focus = focus;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            KeyValueFocus::Key => KeyValueFocus::Value,
            KeyValueFocus::Value => KeyValueFocus::Key,
        };
    }

    pub fn inline_spans(&self) -> Vec<Span> {
        let key_value = self.key();
        let value = self.value_text();
        let cyan = Style::new().color(Color::Cyan);
        let dim = Style::new().color(Color::DarkGrey);

        let key_span = if self.focus == KeyValueFocus::Key {
            Span::styled(key_value, cyan).no_wrap()
        } else {
            Span::styled(key_value, dim).no_wrap()
        };

        let value_span = match &self.value_field {
            ValueField::Select(_) => {
                if self.focus == KeyValueFocus::Value {
                    Span::styled(format!("‹ {value} ›"), cyan).no_wrap()
                } else {
                    Span::styled(format!("‹ {value} ›"), dim).no_wrap()
                }
            }
            ValueField::Text(_) => {
                if self.focus == KeyValueFocus::Value {
                    Span::styled(value, cyan).no_wrap()
                } else {
                    Span::styled(value, dim).no_wrap()
                }
            }
        };

        vec![key_span, Span::new(": ").no_wrap(), value_span]
    }

    fn as_value(&self) -> Value {
        let mut map = IndexMap::new();
        map.insert("key".to_string(), Value::Text(self.key()));
        map.insert("value".to_string(), Value::Text(self.value_text()));
        Value::Object(map)
    }
}

impl Component for KeyValueComponent {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl Drawable for KeyValueComponent {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput {
            lines: vec![self.inline_spans()],
        }
    }
}

impl Interactive for KeyValueComponent {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Tab => {
                self.toggle_focus();
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                InteractionResult::submit_or_produce(self.submit_target.as_ref(), self.as_value())
            }
            _ => match self.focus {
                KeyValueFocus::Key => {
                    self.key_input.on_key(key);
                    InteractionResult::handled()
                }
                KeyValueFocus::Value => match &mut self.value_field {
                    ValueField::Select(select) => match key.code {
                        KeyCode::Left | KeyCode::Right => {
                            select.on_key(key);
                            InteractionResult::handled()
                        }
                        _ => InteractionResult::ignored(),
                    },
                    ValueField::Text(text) => {
                        text.on_key(key);
                        InteractionResult::handled()
                    }
                },
            },
        }
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        match self.focus {
            KeyValueFocus::Key => self.key_input.text_editing(),
            KeyValueFocus::Value => match &mut self.value_field {
                ValueField::Select(_) => None,
                ValueField::Text(text) => text.text_editing(),
            },
        }
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        match self.focus {
            KeyValueFocus::Key => self.key_input.completion(),
            KeyValueFocus::Value => match &mut self.value_field {
                ValueField::Select(_) => None,
                ValueField::Text(text) => text.completion(),
            },
        }
    }

    fn value(&self) -> Option<Value> {
        Some(self.as_value())
    }

    fn set_value(&mut self, value: Value) {
        let Value::Object(map) = value else {
            return;
        };

        if let Some(text) = map.get("key").and_then(Value::to_text_scalar) {
            self.key_input.set_value(Value::Text(text));
        }
        if let Some(text) = map.get("value").and_then(Value::to_text_scalar) {
            match &mut self.value_field {
                ValueField::Select(select) => select.set_value(Value::Text(text)),
                ValueField::Text(input) => input.set_value(Value::Text(text)),
            }
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        Ok(())
    }

    fn cursor_pos(&self) -> Option<crate::terminal::CursorPos> {
        match self.focus {
            KeyValueFocus::Key => self.key_input.cursor_pos(),
            KeyValueFocus::Value => match &self.value_field {
                ValueField::Select(_) => None,
                ValueField::Text(text) => {
                    let key_width: u16 = self
                        .key()
                        .chars()
                        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0) as u16)
                        .sum();
                    let value_cursor = text.cursor_pos()?;
                    Some(crate::terminal::CursorPos {
                        col: key_width.saturating_add(2).saturating_add(value_cursor.col),
                        row: 0,
                    })
                }
            },
        }
    }
}
