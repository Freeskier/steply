use crate::core::node::Node;
use crate::core::validation::FormValidator;

pub struct Step {
    pub prompt: String,
    pub hint: Option<String>,
    pub nodes: Vec<Node>,
    pub form_validators: Vec<FormValidator>,
}
