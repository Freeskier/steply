use crate::node::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMode {
    Modal,
    Shared,
}

pub struct LayerState {
    pub id: String,
    pub mode: LayerMode,
    pub nodes: Vec<Node>,
}

impl LayerState {
    pub fn new(id: impl Into<String>, mode: LayerMode, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            mode,
            nodes,
        }
    }
}

struct ActiveLayer {
    layer: LayerState,
    saved_focus_id: Option<String>,
}

#[derive(Default)]
pub struct LayerManager {
    active: Option<ActiveLayer>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&LayerState> {
        self.active.as_ref().map(|active| &active.layer)
    }

    pub fn active_mut(&mut self) -> Option<&mut LayerState> {
        self.active.as_mut().map(|active| &mut active.layer)
    }

    pub fn open(&mut self, layer: LayerState, saved_focus_id: Option<String>) {
        self.active = Some(ActiveLayer {
            layer,
            saved_focus_id,
        });
    }

    pub fn close(&mut self) -> Option<String> {
        self.active.take().and_then(|active| active.saved_focus_id)
    }
}
