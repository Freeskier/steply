use super::AppState;
use crate::core::NodeId;
use crate::runtime::event::{OverlayLifecycle, WidgetEvent};
use crate::state::overlay::OverlayEntry;
use crate::widgets::node::find_overlay_mut;
use crate::widgets::traits::{FocusMode, OverlayMode};

impl AppState {
    pub fn close_overlay(&mut self) {
        self.clear_completion_session();
        let Some(entry) = self.ui.overlays.close_top() else {
            return;
        };

        let (restored_focus, closed_overlay_id) = {
            let overlay_id = entry.id.as_str();
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(overlay) = find_overlay_mut(nodes, overlay_id) else {
                let restored = entry.focus_before_open.map(|id| id.into_inner());
                self.rebuild_focus_with_target(restored.as_deref(), true);
                return;
            };

            Self::emit_overlay_lifecycle(overlay, overlay_id, OverlayLifecycle::BeforeClose);
            let restored = overlay
                .overlay_close()
                .or_else(|| entry.focus_before_open.map(|id| id.into_inner()));
            Self::emit_overlay_lifecycle(overlay, overlay_id, OverlayLifecycle::Closed);
            (restored, Some(overlay_id.to_string()))
        };

        self.rebuild_focus_with_target(restored_focus.as_deref(), true);

        if let Some(overlay_id) = closed_overlay_id {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            if let Some(overlay) = find_overlay_mut(nodes, &overlay_id) {
                Self::emit_overlay_lifecycle(overlay, &overlay_id, OverlayLifecycle::AfterClose);
            }
        }
    }

    pub fn open_default_overlay(&mut self) -> bool {
        let Some(id) = self.default_overlay_id() else {
            return false;
        };
        self.open_overlay_by_id(&id)
    }

    pub fn open_overlay_by_index(&mut self, index: usize) -> bool {
        let overlays = self.overlay_ids_in_current_step();
        let Some(id) = overlays.get(index) else {
            return false;
        };
        self.open_overlay_by_id(id.as_str())
    }

    pub(super) fn open_overlay_by_id(&mut self, overlay_id: &str) -> bool {
        self.clear_completion_session();
        let saved_focus_id = self.ui.focus.current_id().map(NodeId::from);
        let (opened, focus_mode, overlay_mode) = {
            let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let Some(overlay) = find_overlay_mut(nodes, overlay_id) else {
                return false;
            };
            Self::emit_overlay_lifecycle(overlay, overlay_id, OverlayLifecycle::BeforeOpen);
            let opened = overlay.overlay_open(saved_focus_id.as_ref().map(NodeId::to_string));
            if opened {
                Self::emit_overlay_lifecycle(overlay, overlay_id, OverlayLifecycle::Opened);
            }
            (opened, overlay.focus_mode(), overlay.overlay_mode())
        };
        if opened {
            self.ui.overlays.open(OverlayEntry {
                id: NodeId::from(overlay_id),
                mode: overlay_mode,
                focus_mode,
                focus_before_open: saved_focus_id.clone(),
            });
            let target = match overlay_mode {
                OverlayMode::Exclusive if focus_mode == FocusMode::Group => Some(overlay_id),
                OverlayMode::Shared => saved_focus_id.as_ref().map(NodeId::as_str),
                OverlayMode::Exclusive => None,
            };
            self.rebuild_focus_with_target(target, false);
        }
        opened
    }

    fn emit_overlay_lifecycle(
        overlay: &mut crate::widgets::node::Node,
        overlay_id: &str,
        phase: OverlayLifecycle,
    ) {
        let _ = overlay.on_event(&WidgetEvent::OverlayLifecycle {
            overlay_id: NodeId::from(overlay_id),
            phase,
        });
    }
}
