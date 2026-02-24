use crate::core::NodeId;

#[derive(Debug, Clone)]
pub(in crate::state::app) struct CompletionSession {
    pub owner_id: NodeId,
    pub matches: Vec<String>,
    pub index: usize,
    pub start: usize,
}

impl CompletionSession {
    pub(in crate::state::app) fn new(
        owner_id: NodeId,
        matches: Vec<String>,
        index: usize,
        start: usize,
    ) -> Self {
        Self {
            owner_id,
            matches,
            index,
            start,
        }
    }

    pub(in crate::state::app) fn belongs_to(&self, focused_id: &str) -> bool {
        self.owner_id.as_str() == focused_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::state::app) enum CompletionStartResult {
    None,
    ExpandedToSingle,
    OpenedMenu,
}
