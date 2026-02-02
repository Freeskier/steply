use crate::core::validation::FormValidator;
use crate::node::Node;

pub struct Step {
    pub prompt: String,
    pub hint: Option<String>,
    pub nodes: Vec<Node>,
    pub form_validators: Vec<FormValidator>,
}
