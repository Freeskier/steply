#[derive(Debug, Clone)]
pub struct InputBase {
    id: String,
    label: String,
    focused: bool,
}

impl InputBase {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            focused: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn focus_marker(&self) -> &'static str {
        if self.focused { ">" } else { " " }
    }

    pub fn prefixed_label(&self) -> String {
        format!("{} {}", self.focus_marker(), self.label)
    }
}

#[derive(Debug, Clone)]
pub struct ComponentBase {
    id: String,
    label: String,
    focused: bool,
}

impl ComponentBase {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            focused: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn focus_marker(&self) -> &'static str {
        if self.focused { ">" } else { " " }
    }
}
