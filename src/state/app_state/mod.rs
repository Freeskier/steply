use crate::core::NodeId;
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::flow::Flow;
use crate::state::focus::FocusState;
use crate::state::overlay::OverlayState;
use crate::state::step::Step;
use crate::state::store::ValueStore;
use crate::state::validation::ValidationState;
use crate::widgets::node::{Node, find_overlay, find_overlay_mut, visit_state_nodes};
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};

#[derive(Debug, Clone)]
pub(crate) struct CompletionSession {
    pub owner_id: NodeId,
    pub prefix: String,
    pub matches: Vec<String>,
    pub index: usize,
}

pub struct AppState {
    flow: Flow,
    overlays: OverlayState,
    store: ValueStore,
    validation: ValidationState,
    pending_scheduler: Vec<SchedulerCommand>,
    focus: FocusState,
    completion_session: Option<CompletionSession>,
    should_exit: bool,
}

impl AppState {
    pub fn new(flow: Flow) -> Self {
        let mut state = Self {
            flow,
            overlays: OverlayState::default(),
            store: ValueStore::new(),
            validation: ValidationState::default(),
            pending_scheduler: Vec::new(),
            focus: FocusState::default(),
            completion_session: None,
            should_exit: false,
        };
        state.rebuild_focus();
        state
    }

    pub fn current_step_id(&self) -> &str {
        &self.flow.current_step().id
    }

    pub fn current_step_index(&self) -> usize {
        self.flow.current_index()
    }

    pub fn steps(&self) -> &[Step] {
        self.flow.steps()
    }

    pub fn step_status_at(&self, index: usize) -> crate::state::step::StepStatus {
        self.flow.status_at(index)
    }

    pub fn current_prompt(&self) -> &str {
        &self.flow.current_step().prompt
    }

    pub fn current_hint(&self) -> Option<&str> {
        self.flow.current_step().hint.as_deref()
    }

    pub fn focused_id(&self) -> Option<&str> {
        self.focus.current_id()
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn request_exit(&mut self) {
        self.should_exit = true;
    }

    pub fn active_nodes(&self) -> &[Node] {
        let step_nodes = self.flow.current_step().nodes.as_slice();
        if let Some(entry) = self.overlays.active_blocking()
            && let Some(overlay) = find_overlay(step_nodes, entry.id.as_str())
        {
            if overlay.focus_mode() == FocusMode::Group {
                return step_nodes;
            }
            if let Some(children) = overlay.children() {
                return children;
            }
        }
        step_nodes
    }

    pub fn active_nodes_mut(&mut self) -> &mut [Node] {
        let active_blocking = self.overlays.active_blocking().cloned();
        if let Some(active_blocking) = active_blocking {
            if active_blocking.focus_mode == FocusMode::Group {
                return self.flow.current_step_mut().nodes.as_mut_slice();
            }
            let step_nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let overlay = find_overlay_mut(step_nodes, active_blocking.id.as_str())
                .expect("active overlay id should resolve to an overlay node");
            return overlay
                .children_mut()
                .expect("active overlay should expose active children");
        }
        self.flow.current_step_mut().nodes.as_mut_slice()
    }

    pub fn has_active_overlay(&self) -> bool {
        self.active_overlay().is_some()
    }

    pub fn active_overlay_id(&self) -> Option<&str> {
        self.active_overlay().map(Node::id)
    }

    pub fn active_overlay(&self) -> Option<&Node> {
        let overlay_id = self.overlays.active_id()?;
        find_overlay(
            self.flow.current_step().nodes.as_slice(),
            overlay_id.as_str(),
        )
    }

    pub fn overlay_by_id(&self, id: &NodeId) -> Option<&Node> {
        find_overlay(self.flow.current_step().nodes.as_slice(), id.as_str())
    }

    pub fn overlay_stack_ids(&self) -> Vec<NodeId> {
        self.overlays
            .entries()
            .iter()
            .map(|entry| entry.id.clone())
            .collect()
    }

    pub fn active_overlay_nodes(&self) -> Option<&[Node]> {
        self.active_overlay().and_then(Node::children)
    }

    pub fn active_overlay_placement(&self) -> Option<OverlayPlacement> {
        self.active_overlay().and_then(Node::overlay_placement)
    }

    pub fn active_overlay_focus_mode(&self) -> Option<FocusMode> {
        self.overlays.active().map(|entry| entry.focus_mode)
    }

    pub fn active_overlay_mode(&self) -> Option<OverlayMode> {
        self.overlays.active().map(|entry| entry.mode)
    }

    pub fn has_blocking_overlay(&self) -> bool {
        self.overlays.active_blocking().is_some()
    }

    pub fn default_overlay_id(&self) -> Option<String> {
        self.overlay_ids_in_current_step()
            .into_iter()
            .next()
            .map(NodeId::into_inner)
    }

    pub fn overlay_ids_in_current_step(&self) -> Vec<NodeId> {
        let mut ids = Vec::<NodeId>::new();
        visit_state_nodes(self.flow.current_step().nodes.as_slice(), &mut |node| {
            if node.overlay_placement().is_some() {
                ids.push(node.id().into());
            }
        });
        ids
    }

    pub fn current_step_nodes(&self) -> &[Node] {
        self.flow.current_step().nodes.as_slice()
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.validation.visible_error(id)
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.validation.is_hidden_invalid(id)
    }

    pub fn clear_step_errors(&mut self) {
        self.validation.clear_step_errors();
    }

    pub fn current_step_errors(&self) -> &[String] {
        self.validation.step_errors()
    }
}

mod navigation;
mod validation_runtime;
mod value_sync;
