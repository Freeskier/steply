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

#[derive(Default)]
pub struct LayerManager {
    active: Option<LayerState>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&LayerState> {
        self.active.as_ref()
    }

    pub fn active_mut(&mut self) -> Option<&mut LayerState> {
        self.active.as_mut()
    }

    pub fn open(&mut self, layer: LayerState) {
        self.active = Some(layer);
    }

    pub fn close(&mut self) {
        self.active = None;
    }
}
