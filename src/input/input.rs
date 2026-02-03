use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::validators::Validator;

pub type NodeId = String;

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
    fn id(&self) -> &NodeId;
    fn label(&self) -> &str;
    fn value(&self) -> String;
    fn set_value(&mut self, value: String);
    fn raw_value(&self) -> String {
        self.value()
    }
    fn is_complete(&self) -> bool {
        true
    }
    fn capabilities(&self) -> InputCaps {
        InputCaps::default()
    }

    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);

    fn error(&self) -> Option<&str>;
    fn set_error(&mut self, error: Option<String>);

    fn cursor_pos(&self) -> usize;
    fn min_width(&self) -> usize;

    fn validators(&self) -> &[Validator];

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

    fn render_content(&self) -> Vec<Span>;

    fn cursor_offset_in_content(&self) -> usize;

    fn delete_word(&mut self) {}
    fn delete_word_forward(&mut self) {}
}

pub struct InputBase {
    pub id: NodeId,
    pub label: String,
    pub focused: bool,
    pub error: Option<String>,
    pub validators: Vec<Validator>,
    pub min_width: usize,
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
}
