use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorVisibility {
    Hidden,
    Inline,
    StepSummary,
}

#[derive(Debug, Clone)]
pub struct ValidationEntry {
    pub error: String,
    pub visibility: ErrorVisibility,
}

#[derive(Debug, Default, Clone)]
pub struct ValidationState {
    entries: HashMap<String, ValidationEntry>,
}

impl ValidationState {
    pub fn set_error(
        &mut self,
        id: impl Into<String>,
        error: impl Into<String>,
        visibility: ErrorVisibility,
    ) {
        self.entries.insert(
            id.into(),
            ValidationEntry {
                error: error.into(),
                visibility,
            },
        );
    }

    pub fn clear_error(&mut self, id: &str) {
        self.entries.remove(id);
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.entries.get(id).and_then(|entry| {
            matches!(
                entry.visibility,
                ErrorVisibility::Inline | ErrorVisibility::StepSummary
            )
            .then_some(entry.error.as_str())
        })
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.entries
            .get(id)
            .is_some_and(|entry| matches!(entry.visibility, ErrorVisibility::Hidden))
    }

    pub fn set_visibility(&mut self, id: &str, visibility: ErrorVisibility) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.visibility = visibility;
        }
    }

    pub fn visible_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().filter_map(|(id, entry)| {
            matches!(
                entry.visibility,
                ErrorVisibility::Inline | ErrorVisibility::StepSummary
            )
            .then_some((id.as_str(), entry.error.as_str()))
        })
    }

    pub fn clear_for_ids(&mut self, allowed_ids: &[String]) {
        self.entries
            .retain(|id, _| allowed_ids.iter().any(|allowed| allowed == id));
    }
}
