use crate::core::node::NodeId;
use crate::core::validation::FormValidator;

pub struct Step {
    pub prompt: String,
    pub hint: Option<String>,
    pub node_ids: Vec<NodeId>,
    pub form_validators: Vec<FormValidator>,
}
