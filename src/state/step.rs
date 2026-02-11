use crate::node::Node;

pub struct Step {
    pub id: String,
    pub prompt: String,
    pub hint: Option<String>,
    pub nodes: Vec<Node>,
}

impl Step {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            hint: None,
            nodes,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}
