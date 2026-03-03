use super::CompletionSession;
use crate::core::NodeId;
use crate::state::app::AppState;

impl AppState {
    pub(in crate::state::app) fn clear_completion_session(&mut self) {
        self.ui.completion_session = None;
    }

    pub(in crate::state::app) fn reset_completion_for_focus_change(&mut self) {
        self.clear_completion_session();
        self.ui.completion_tab_suppressed_for = None;
    }

    pub(crate) fn suppress_completion_tab_for_focused(&mut self) {
        let Some(focused_id) = self.focused_id() else {
            return;
        };
        self.ui.completion_tab_suppressed_for = Some(NodeId::from(focused_id));
    }

    pub(in crate::state::app) fn clear_completion_tab_suppression_for_focused(&mut self) {
        let Some(focused_id) = self.focused_id() else {
            self.ui.completion_tab_suppressed_for = None;
            return;
        };
        if self
            .ui
            .completion_tab_suppressed_for
            .as_ref()
            .is_some_and(|id| id.as_str() == focused_id)
        {
            self.ui.completion_tab_suppressed_for = None;
        }
    }

    pub(in crate::state::app) fn is_completion_tab_suppressed_for_focused(&self) -> bool {
        let Some(focused_id) = self.focused_id() else {
            return false;
        };
        self.ui
            .completion_tab_suppressed_for
            .as_ref()
            .is_some_and(|id| id.as_str() == focused_id)
    }

    pub(crate) fn has_completion_for_focused(&self) -> bool {
        let Some(session) = self.ui.completion_session.as_ref() else {
            return false;
        };
        self.focused_id()
            .is_some_and(|focused| session.belongs_to(focused))
    }

    pub(crate) fn completion_match_count_for_focused(&self) -> Option<usize> {
        let session = self.ui.completion_session.as_ref()?;
        let focused = self.focused_id()?;
        session.belongs_to(focused).then_some(session.matches.len())
    }

    pub(crate) fn cancel_completion_for_focused(&mut self) -> bool {
        if self.has_completion_for_focused() {
            self.clear_completion_session();
            return true;
        }
        false
    }

    pub(crate) fn toggle_completion_for_focused(&mut self) {
        if self.cancel_completion_for_focused() {
            self.suppress_completion_tab_for_focused();
            return;
        }

        self.clear_completion_tab_suppression_for_focused();
        let Some(focused_id) = self.focused_id_owned() else {
            return;
        };

        let session = self
            .focused_completion_data(&focused_id)
            .and_then(|completion| {
                if completion.matches.is_empty() {
                    return None;
                }
                Some(CompletionSession::new(
                    NodeId::from(focused_id.as_str()),
                    completion.matches,
                    0,
                    completion.query.start,
                ))
            });

        self.ui.completion_session = session;
    }
}
