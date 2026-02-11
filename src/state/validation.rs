use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationEntry {
    pub error: String,
    pub revealed: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ValidationState {
    entries: HashMap<String, ValidationEntry>,
}

impl ValidationState {
    pub fn set_error(&mut self, id: impl Into<String>, error: impl Into<String>, revealed: bool) {
        self.entries.insert(
            id.into(),
            ValidationEntry {
                error: error.into(),
                revealed,
            },
        );
    }

    pub fn clear_error(&mut self, id: &str) {
        self.entries.remove(id);
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.entries
            .get(id)
            .and_then(|entry| entry.revealed.then_some(entry.error.as_str()))
    }

    pub fn set_revealed(&mut self, id: &str, revealed: bool) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.revealed = revealed;
        }
    }

    pub fn clear_for_ids(&mut self, allowed_ids: &[String]) {
        self.entries
            .retain(|id, _| allowed_ids.iter().any(|allowed| allowed == id));
    }
}
