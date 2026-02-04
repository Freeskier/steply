use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::theme::Theme;
use crate::validators::Validator;
use crate::value::Value;

pub type NodeId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    pub message: String,
    pub display: ErrorDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorDisplay {
    #[default]
    Inline,
    Hidden,
}

impl InputError {
    pub fn inline(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            display: ErrorDisplay::Inline,
        }
    }

    pub fn hidden(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            display: ErrorDisplay::Hidden,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.display == ErrorDisplay::Inline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    Handled,
    NotHandled,
    Submit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputCaps {
    pub capture_tab: bool,
    pub capture_backtab: bool,
    pub capture_ctrl_backspace: bool,
    pub capture_ctrl_delete: bool,
    pub capture_ctrl_left: bool,
    pub capture_ctrl_right: bool,
}

impl InputCaps {
    pub fn captures_key(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match (code, modifiers) {
            (KeyCode::Tab, mods) if mods == KeyModifiers::NONE => self.capture_tab,
            (KeyCode::BackTab, mods) if mods.contains(KeyModifiers::SHIFT) => self.capture_backtab,
            (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::CONTROL) => {
                self.capture_ctrl_backspace
            }
            (KeyCode::Delete, mods) if mods.contains(KeyModifiers::CONTROL) => {
                self.capture_ctrl_delete
            }
            (KeyCode::Left, mods) if mods.contains(KeyModifiers::CONTROL) => self.capture_ctrl_left,
            (KeyCode::Right, mods) if mods.contains(KeyModifiers::CONTROL) => {
                self.capture_ctrl_right
            }
            _ => false,
        }
    }
}

pub trait Input: Send {
    fn base(&self) -> &InputBase;
    fn base_mut(&mut self) -> &mut InputBase;

    fn id(&self) -> &NodeId {
        &self.base().id
    }

    fn label(&self) -> &str {
        &self.base().label
    }

    fn value(&self) -> String;
    fn set_value(&mut self, value: String);
    fn value_typed(&self) -> Value {
        Value::Text(self.value())
    }
    fn set_value_typed(&mut self, value: Value) {
        match value {
            Value::Text(text) => self.set_value(text),
            Value::Bool(val) => self.set_value(if val { "true" } else { "false" }.to_string()),
            Value::Number(num) => self.set_value(num.to_string()),
            Value::List(items) => self.set_value(items.join(",")),
            Value::Map(items) => {
                let joined = items
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",");
                self.set_value(joined);
            }
            Value::None => self.set_value(String::new()),
        }
    }
    fn raw_value(&self) -> String {
        self.value()
    }
    fn is_complete(&self) -> bool {
        true
    }
    fn capabilities(&self) -> InputCaps {
        InputCaps::default()
    }

    fn is_focused(&self) -> bool {
        self.base().focused
    }

    fn set_focused(&mut self, focused: bool) {
        let base = self.base_mut();
        base.focused = focused;
        if !focused {
            base.error = None;
        }
    }

    fn error(&self) -> Option<&InputError> {
        self.base().error.as_ref()
    }

    fn set_error(&mut self, error: Option<InputError>) {
        self.base_mut().error = error;
    }

    fn clear_error(&mut self) {
        self.base_mut().error = None;
    }

    fn has_visible_error(&self) -> bool {
        self.error().map(|e| e.is_visible()).unwrap_or(false)
    }

    fn cursor_pos(&self) -> usize;
    fn min_width(&self) -> usize {
        self.base().min_width
    }

    fn validators(&self) -> &[Validator] {
        &self.base().validators
    }

    fn supports_tab_completion(&self) -> bool {
        false
    }

    fn handle_tab_completion(&mut self) -> bool {
        false
    }

    fn render_brackets(&self) -> bool {
        true
    }

    fn placeholder(&self) -> Option<&str> {
        self.base().placeholder.as_deref()
    }

    fn validate(&self) -> Result<(), String> {
        for validator in self.validators() {
            validator(&self.value())?;
        }
        Ok(())
    }

    fn validate_internal(&self) -> Result<(), String> {
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult;

    fn render_content(&self, theme: &Theme) -> Vec<Span>;

    fn cursor_offset_in_content(&self) -> usize;

    fn delete_word(&mut self) {}
    fn delete_word_forward(&mut self) {}
}

pub struct InputBase {
    pub id: NodeId,
    pub label: String,
    pub focused: bool,
    pub error: Option<InputError>,
    pub validators: Vec<Validator>,
    pub min_width: usize,
    pub placeholder: Option<String>,
}

impl InputBase {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            focused: false,
            error: None,
            validators: Vec::new(),
            min_width: 1,
            placeholder: None,
        }
    }

    pub fn with_min_width(mut self, width: usize) -> Self {
        self.min_width = width;
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}
