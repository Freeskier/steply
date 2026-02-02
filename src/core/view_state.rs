use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorDisplay {
    None,
    InlineMessage,
}

#[derive(Debug, Default)]
pub struct ViewState {
    error_display: HashMap<String, ErrorDisplay>,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
            error_display: HashMap::new(),
        }
    }

    pub fn error_display(&self, id: &str) -> ErrorDisplay {
        self.error_display
            .get(id)
            .copied()
            .unwrap_or(ErrorDisplay::None)
    }

    pub fn set_error_display(&mut self, id: String, display: ErrorDisplay) {
        if display == ErrorDisplay::None {
            self.error_display.remove(&id);
        } else {
            self.error_display.insert(id, display);
        }
    }

    pub fn clear_error_display(&mut self, id: &str) {
        self.error_display.remove(id);
    }
}
