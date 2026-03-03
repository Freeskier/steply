use crate::runtime::event::SystemEvent;
use crate::state::app::AppState;
use crate::state::focus::FocusState;
use crate::widgets::node_index::NodeIndex;
use crate::widgets::traits::FocusMode;

impl AppState {
    pub fn focus_next(&mut self) {
        self.reset_completion_for_focus_change();
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused_live();
        self.ui.focus.next();
        self.broadcast_current_focus_request();
    }

    pub fn focus_prev(&mut self) {
        self.reset_completion_for_focus_change();
        if self.has_blocking_overlay()
            && matches!(self.active_overlay_focus_mode(), Some(FocusMode::Group))
        {
            return;
        }
        self.validate_focused_live();
        self.ui.focus.prev();
        self.broadcast_current_focus_request();
    }

    pub(in crate::state::app) fn rebuild_focus_with_target(
        &mut self,
        target: Option<&str>,
        prune_validation: bool,
    ) {
        self.reset_completion_for_focus_change();
        self.ui.active_node_index = NodeIndex::build(self.active_nodes());
        self.ui.focus = FocusState::from_nodes(self.active_nodes());
        if let Some(id) = target
            && self.ui.active_node_index.has_visible(id)
        {
            self.ui.focus.set_focus_by_id(id);
        }
        if prune_validation {
            self.prune_validation_for_active_nodes();
        }
        self.broadcast_current_focus_request();
    }

    pub(in crate::state::app) fn rebuild_focus(&mut self) {
        self.rebuild_focus_with_target(None, true);
    }

    fn broadcast_current_focus_request(&mut self) {
        let focused_id = self.ui.focus.current_id().map(|id| id.into());
        let result = self.broadcast_system_event(&SystemEvent::RequestFocus { target: focused_id });
        self.process_broadcast_result(result);
    }
}
