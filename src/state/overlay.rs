use crate::core::NodeId;
use crate::widgets::traits::{FocusMode, OverlayMode};

#[derive(Debug, Clone)]
pub struct OverlayEntry {
    pub id: NodeId,
    pub mode: OverlayMode,
    pub focus_mode: FocusMode,
    pub focus_before_open: Option<NodeId>,
}

#[derive(Debug, Default, Clone)]
pub struct OverlayState {
    stack: Vec<OverlayEntry>,
}

impl OverlayState {
    pub fn open(&mut self, entry: OverlayEntry) {
        self.stack.retain(|current| current.id != entry.id);
        self.stack.push(entry);
    }

    pub fn close_top(&mut self) -> Option<OverlayEntry> {
        self.stack.pop()
    }

    pub fn close_by_id(&mut self, id: &NodeId) -> Option<OverlayEntry> {
        let idx = self.stack.iter().position(|entry| &entry.id == id)?;
        Some(self.stack.remove(idx))
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn active(&self) -> Option<&OverlayEntry> {
        self.stack.last()
    }

    pub fn active_id(&self) -> Option<&NodeId> {
        self.active().map(|entry| &entry.id)
    }

    pub fn active_blocking(&self) -> Option<&OverlayEntry> {
        self.stack
            .iter()
            .rev()
            .find(|entry| matches!(entry.mode, OverlayMode::Exclusive))
    }

    pub fn active_blocking_id(&self) -> Option<&NodeId> {
        self.active_blocking().map(|entry| &entry.id)
    }

    pub fn entries(&self) -> &[OverlayEntry] {
        self.stack.as_slice()
    }
}
