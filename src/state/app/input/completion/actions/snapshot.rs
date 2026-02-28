use crate::state::app::AppState;
use crate::widgets::node::find_node_mut;
use crate::widgets::shared::text_edit;

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
        let nodes = self.active_nodes_mut();
        let Some(node) = find_node_mut(nodes, &focused_id) else {
            return false;
        };
        let Some(state) = node.completion() else {
            return false;
        };
        *state.cursor >= text_edit::char_count(state.value.as_str())
    }
}
