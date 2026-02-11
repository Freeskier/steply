use crate::components::select_component::SelectComponent;
use crate::core::component::Component;
use crate::core::value::Value;
use crate::inputs::Input;
use crate::inputs::text_input::TextInput;

use super::FileBrowserState;

impl FileBrowserState {
    pub fn input_id(&self) -> &str {
        &self.input.base_ref().id
    }

    pub fn list_id(&self) -> &str {
        self.select.id()
    }

    pub fn input(&self) -> &TextInput {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut TextInput {
        &mut self.input
    }

    pub fn select(&self) -> &SelectComponent {
        &self.select
    }

    pub fn select_mut(&mut self) -> &mut SelectComponent {
        &mut self.select
    }

    pub fn selected_value(&self) -> Option<Value> {
        self.selected_entry()
            .map(|entry| Value::Text(entry.path.to_string_lossy().to_string()))
    }

    pub fn set_value(&mut self, value: Value) {
        if let Value::Text(text) = value {
            self.input.set_value(text);
            self.refresh_view();
        }
    }
}
