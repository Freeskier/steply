use crate::core::NodeId;
use crate::widgets::node::{Node, NodeWalkScope, find_overlay, find_overlay_mut, walk_nodes};
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};

use super::AppState;

impl AppState {
    pub fn active_nodes(&self) -> &[Node] {
        if self.flow.is_empty() {
            return &[];
        }
        let step_nodes = self.flow.current_step().nodes.as_slice();
        let Some((overlay_id, focus_mode)) = self.active_blocking_overlay_info() else {
            return step_nodes;
        };
        if focus_mode == FocusMode::Group {
            return step_nodes;
        }
        if let Some(children) =
            find_overlay(step_nodes, overlay_id.as_str()).and_then(Node::persistent_children)
        {
            return children;
        }
        step_nodes
    }

    pub fn active_nodes_mut(&mut self) -> &mut [Node] {
        if self.flow.is_empty() {
            return self.scratch_nodes.as_mut_slice();
        }
        let Some((overlay_id, focus_mode)) = self.active_blocking_overlay_info() else {
            return self.flow.current_step_mut().nodes.as_mut_slice();
        };
        if focus_mode == FocusMode::Group {
            return self.flow.current_step_mut().nodes.as_mut_slice();
        }

        if self.overlay_has_persistent_children(overlay_id.as_str()) {
            let step_nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            if let Some(overlay) = find_overlay_mut(step_nodes, overlay_id.as_str())
                && let Some(children) = overlay.persistent_children_mut()
            {
                return children;
            }
            return self.scratch_nodes.as_mut_slice();
        }

        self.flow.current_step_mut().nodes.as_mut_slice()
    }

    pub fn clean_broken_overlays(&mut self) {
        let Some((overlay_id, focus_mode)) = self.active_blocking_overlay_info() else {
            return;
        };
        if focus_mode == FocusMode::Group {
            return;
        }
        if !self.overlay_has_persistent_children(overlay_id.as_str()) {
            self.ui.overlays.clear();
        }
    }

    pub fn has_active_overlay(&self) -> bool {
        self.active_overlay().is_some()
    }

    pub fn active_overlay_id(&self) -> Option<&str> {
        self.active_overlay().map(Node::id)
    }

    pub fn active_overlay(&self) -> Option<&Node> {
        if self.flow.is_empty() {
            return None;
        }
        let overlay_id = self.ui.overlays.active_id()?;
        find_overlay(
            self.flow.current_step().nodes.as_slice(),
            overlay_id.as_str(),
        )
    }

    pub fn overlay_by_id(&self, id: &NodeId) -> Option<&Node> {
        if self.flow.is_empty() {
            return None;
        }
        find_overlay(self.flow.current_step().nodes.as_slice(), id.as_str())
    }

    pub fn overlay_stack_ids(&self) -> Vec<NodeId> {
        self.ui
            .overlays
            .entries()
            .iter()
            .map(|entry| entry.id.clone())
            .collect()
    }

    pub fn active_overlay_nodes(&self) -> Option<&[Node]> {
        self.active_overlay().and_then(Node::persistent_children)
    }

    pub fn active_overlay_placement(&self) -> Option<OverlayPlacement> {
        self.active_overlay().and_then(Node::overlay_placement)
    }

    pub fn active_overlay_focus_mode(&self) -> Option<FocusMode> {
        self.ui.overlays.active().map(|entry| entry.focus_mode)
    }

    pub fn active_overlay_mode(&self) -> Option<OverlayMode> {
        self.ui.overlays.active().map(|entry| entry.mode)
    }

    pub fn has_blocking_overlay(&self) -> bool {
        self.ui.overlays.active_blocking().is_some()
    }

    pub fn default_overlay_id(&self) -> Option<String> {
        self.overlay_ids_in_current_step()
            .into_iter()
            .next()
            .map(NodeId::into_inner)
    }

    pub fn overlay_ids_in_current_step(&self) -> Vec<NodeId> {
        if self.flow.is_empty() {
            return Vec::new();
        }
        let mut ids = Vec::<NodeId>::new();
        walk_nodes(
            self.flow.current_step().nodes.as_slice(),
            NodeWalkScope::Recursive,
            &mut |node| {
                if node.overlay_placement().is_some() {
                    ids.push(node.id().into());
                }
            },
        );
        ids
    }

    pub fn current_step_nodes(&self) -> &[Node] {
        if self.flow.is_empty() {
            return &[];
        }
        self.flow.current_step().nodes.as_slice()
    }
}
