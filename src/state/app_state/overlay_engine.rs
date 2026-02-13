use crate::core::NodeId;
use crate::state::overlay::{OverlayEntry, OverlayState};

#[derive(Debug, Default, Clone)]
pub(crate) struct OverlayEngine {
    state: OverlayState,
}

impl OverlayEngine {
    pub fn open(&mut self, entry: OverlayEntry) {
        self.state.open(entry);
    }

    pub fn close_top(&mut self) -> Option<OverlayEntry> {
        self.state.close_top()
    }

    pub fn clear(&mut self) {
        self.state.clear();
    }

    pub fn active(&self) -> Option<&OverlayEntry> {
        self.state.active()
    }

    pub fn active_id(&self) -> Option<&NodeId> {
        self.state.active_id()
    }

    pub fn active_blocking(&self) -> Option<&OverlayEntry> {
        self.state.active_blocking()
    }

    pub fn entries(&self) -> &[OverlayEntry] {
        self.state.entries()
    }
}
