use crate::core::node::NodeId;

#[derive(Debug, Clone)]
pub enum FormEvent {
    InputChanged {
        id: NodeId,
        value: String,
    },
    FocusChanged {
        from: Option<NodeId>,
        to: Option<NodeId>,
    },
    SubmitRequested,
    ErrorScheduled {
        id: NodeId,
    },
    ErrorCancelled {
        id: NodeId,
    },
}
