use crate::runtime::scheduler::SchedulerCommand;
use crate::state::flow::Flow;
use crate::state::focus::FocusState;
use crate::state::step::Step;
use crate::state::store::ValueStore;
use crate::state::validation::ValidationState;
use crate::widgets::node::{Node, find_overlay_mut, find_visible_overlay};
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};

pub struct AppState {
    flow: Flow,
    store: ValueStore,
    validation: ValidationState,
    pending_scheduler: Vec<SchedulerCommand>,
    focus: FocusState,
    should_exit: bool,
}

impl AppState {
    pub fn new(flow: Flow) -> Self {
        let mut state = Self {
            flow,
            store: ValueStore::new(),
            validation: ValidationState::default(),
            pending_scheduler: Vec::new(),
            focus: FocusState::default(),
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
        if let Some(overlay) = find_visible_overlay(step_nodes) {
            if overlay.overlay_mode() == OverlayMode::Shared {
                return step_nodes;
            }
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
        let active_overlay_id = self
            .active_overlay()
            .map(|overlay| overlay.id().to_string());
        let active_overlay_mode = self.active_overlay_mode();
        let active_overlay_focus_mode = self.active_overlay_focus_mode();
        if let Some(overlay_id) = active_overlay_id {
            if active_overlay_mode == Some(OverlayMode::Shared) {
                return self.flow.current_step_mut().nodes.as_mut_slice();
            }
            if active_overlay_focus_mode == Some(FocusMode::Group) {
                return self.flow.current_step_mut().nodes.as_mut_slice();
            }
            let step_nodes = self.flow.current_step_mut().nodes.as_mut_slice();
            let overlay = find_overlay_mut(step_nodes, &overlay_id)
                .expect("active overlay id should resolve to an overlay node");
            return overlay
                .children_mut()
                .expect("active overlay should expose active children");
        }
        self.flow.current_step_mut().nodes.as_mut_slice()
    }

    pub fn has_active_overlay(&self) -> bool {
        find_visible_overlay(self.flow.current_step().nodes.as_slice()).is_some()
    }

    pub fn active_overlay(&self) -> Option<&Node> {
        find_visible_overlay(self.flow.current_step().nodes.as_slice())
    }

    pub fn active_overlay_nodes(&self) -> Option<&[Node]> {
        self.active_overlay().and_then(Node::children)
    }

    pub fn active_overlay_placement(&self) -> Option<OverlayPlacement> {
        self.active_overlay().and_then(Node::overlay_placement)
    }

    pub fn active_overlay_focus_mode(&self) -> Option<FocusMode> {
        self.active_overlay().map(Node::focus_mode)
    }

    pub fn active_overlay_mode(&self) -> Option<OverlayMode> {
        self.active_overlay().map(Node::overlay_mode)
    }

    pub fn has_blocking_overlay(&self) -> bool {
        matches!(self.active_overlay_mode(), Some(OverlayMode::Exclusive))
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
