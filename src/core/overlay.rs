use crate::node::Node;
use crate::text_input::TextInput;

#[derive(Clone, Debug)]
pub struct OverlayState {
    pub label: String,
    pub hint: Option<String>,
    pub input_ids: Vec<String>,
}

impl OverlayState {
    pub fn demo() -> (Self, Vec<Node>) {
        let input_id = "overlay_query".to_string();
        let nodes = vec![Node::overlay_input(TextInput::new(&input_id, "Search"))];
        let overlay = Self {
            label: "Overlay demo: type, Esc to close".to_string(),
            hint: None,
            input_ids: vec![input_id],
        };
        (overlay, nodes)
    }
}
