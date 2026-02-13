use crate::state::focus::FocusState;
use crate::widgets::node::Node;

#[derive(Debug, Default, Clone)]
pub(crate) struct FocusEngine {
    state: FocusState,
}

impl FocusEngine {
    pub fn from_nodes(nodes: &[Node]) -> Self {
        Self {
            state: FocusState::from_nodes(nodes),
        }
    }

    pub fn current_id(&self) -> Option<&str> {
        self.state.current_id()
    }

    pub fn set_focus_by_id(&mut self, id: &str) {
        self.state.set_focus_by_id(id);
    }

    pub fn next(&mut self) {
        self.state.next();
    }

    pub fn prev(&mut self) {
        self.state.prev();
    }
}
