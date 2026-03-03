use crate::state::app::AppState;

impl AppState {
    pub fn completion_snapshot(&self) -> Option<(String, Vec<String>, usize, usize)> {
        let session = self.ui.completion_session.as_ref()?;
        let focused = self.focused_id()?;
        if !session.belongs_to(focused) {
            return None;
        }
        Some((
            session.owner_id.to_string(),
            session.matches.clone(),
            session.index,
            session.start,
        ))
    }

    pub(crate) fn cursor_at_end_for_focused(&mut self) -> bool {
        let Some(focused_id) = self.focused_id_owned() else {
            return false;
        };
        self.cursor_at_end_in_focused(&focused_id)
    }
}
