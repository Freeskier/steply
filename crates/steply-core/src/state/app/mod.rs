use crate::core::NodeId;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::state::validation::ValidationState;
use crate::task::{TaskId, TaskSpec, TaskSubscription};
use crate::widgets::node::{Node, NodeWalkScope, find_overlay, find_overlay_mut, walk_nodes};
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};
use std::collections::HashMap;

use self::state::{DataState, RuntimeState, ViewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitConfirmChoice {
    Stay,
    Exit,
}

pub struct AppState {
    flow: Flow,
    ui: ViewState,
    data: DataState,
    runtime: RuntimeState,
    scratch_nodes: Vec<Node>,
    should_exit: bool,
    pending_back_confirm: Option<String>,
    pending_exit_confirm: Option<ExitConfirmChoice>,
}

impl AppState {
    pub fn new(flow: Flow) -> Self {
        Self::with_tasks(flow, Vec::new(), Vec::new())
    }

    pub fn with_tasks(
        flow: Flow,
        task_specs: Vec<TaskSpec>,
        task_subscriptions: Vec<TaskSubscription>,
    ) -> Self {
        let (inline_specs, inline_subscriptions) = collect_inline_tasks(&flow);
        let mut spec_map = HashMap::<TaskId, TaskSpec>::new();
        for spec in inline_specs {
            spec_map.insert(spec.id.clone(), spec);
        }
        for spec in task_specs {
            spec_map.insert(spec.id.clone(), spec);
        }
        let mut subscriptions = inline_subscriptions;
        subscriptions.extend(task_subscriptions);

        let mut state = Self {
            flow,
            ui: ViewState::default(),
            data: DataState::default(),
            runtime: RuntimeState::with_tasks(spec_map, subscriptions),
            scratch_nodes: Vec::new(),
            should_exit: false,
            pending_back_confirm: None,
            pending_exit_confirm: None,
        };
        if state.flow.is_empty() {
            state.should_exit = true;
        } else {
            state.reconcile_current_step_visibility();
            state.rebuild_focus();
            crate::task::engine::trigger_flow_start_tasks(&mut state);
            let current_step_id = state.current_step_id().to_string();
            crate::task::engine::trigger_step_enter_tasks(&mut state, current_step_id.as_str());
            crate::task::engine::bootstrap_interval_tasks(&mut state);
        }
        state
    }

    pub fn current_step_id(&self) -> &str {
        if self.flow.is_empty() {
            return "";
        }
        &self.flow.current_step().id
    }

    pub fn current_step_index(&self) -> usize {
        self.flow.current_index()
    }

    pub fn current_visible_step_index(&self) -> usize {
        let visible = self.visible_step_indices();
        visible
            .iter()
            .position(|&index| index == self.flow.current_index())
            .unwrap_or(0)
    }

    pub fn steps(&self) -> &[Step] {
        self.flow.steps()
    }

    pub fn step_index_by_id(&self, step_id: &str) -> Option<usize> {
        self.flow.steps().iter().position(|step| step.id == step_id)
    }

    pub fn set_current_step_for_preview(&mut self, index: usize) -> bool {
        if !self.flow.set_current(index) {
            return false;
        }
        self.prepare_current_step_for_preview();
        true
    }

    pub fn set_current_step_by_id_for_preview(&mut self, step_id: &str) -> bool {
        let Some(index) = self.step_index_by_id(step_id) else {
            return false;
        };
        self.set_current_step_for_preview(index)
    }

    pub fn step_status_at(&self, index: usize) -> crate::state::step::StepStatus {
        self.flow.status_at(index)
    }

    pub fn step_visible_at(&self, index: usize) -> bool {
        self.flow
            .steps()
            .get(index)
            .is_some_and(|step| step.is_visible(&self.data.store))
    }

    pub fn visible_step_indices(&self) -> Vec<usize> {
        self.flow
            .steps()
            .iter()
            .enumerate()
            .filter_map(|(index, step)| step.is_visible(&self.data.store).then_some(index))
            .collect()
    }

    pub(super) fn reconcile_current_step_visibility(&mut self) {
        if self.flow.is_empty() || self.step_visible_at(self.flow.current_index()) {
            return;
        }

        while self.flow.advance() {
            if self.step_visible_at(self.flow.current_index()) {
                return;
            }
        }

        while self.flow.go_back() {
            if self.step_visible_at(self.flow.current_index()) {
                return;
            }
        }
    }

    pub fn current_prompt(&self) -> &str {
        if self.flow.is_empty() {
            return "";
        }
        &self.flow.current_step().prompt
    }

    pub fn current_description(&self) -> Option<&str> {
        if self.flow.is_empty() {
            return None;
        }
        self.flow.current_step().description.as_deref()
    }

    pub fn hints_visible(&self) -> bool {
        self.ui.hints_visible
    }

    pub fn toggle_hints_visibility(&mut self) {
        self.ui.hints_visible = !self.ui.hints_visible;
    }

    pub fn focused_id(&self) -> Option<&str> {
        self.ui.focus.current_id()
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn back_confirm(&self) -> Option<&str> {
        self.pending_back_confirm.as_deref()
    }

    pub fn exit_confirm_choice(&self) -> Option<ExitConfirmChoice> {
        self.pending_exit_confirm
    }

    pub fn exit_confirm_active(&self) -> bool {
        self.pending_exit_confirm.is_some()
    }

    pub fn begin_exit_confirm(&mut self) {
        self.pending_exit_confirm = Some(ExitConfirmChoice::Stay);
    }

    pub fn cancel_exit_confirm(&mut self) {
        self.pending_exit_confirm = None;
    }

    pub fn toggle_exit_confirm_choice(&mut self) -> bool {
        let Some(choice) = self.pending_exit_confirm else {
            return false;
        };
        self.pending_exit_confirm = Some(match choice {
            ExitConfirmChoice::Stay => ExitConfirmChoice::Exit,
            ExitConfirmChoice::Exit => ExitConfirmChoice::Stay,
        });
        true
    }

    pub fn set_exit_confirm_choice(&mut self, choice: ExitConfirmChoice) -> bool {
        let Some(current) = self.pending_exit_confirm else {
            return false;
        };
        if current == choice {
            return false;
        }
        self.pending_exit_confirm = Some(choice);
        true
    }

    pub fn resolve_exit_confirm(&mut self) -> bool {
        let Some(choice) = self.pending_exit_confirm.take() else {
            return false;
        };
        if choice == ExitConfirmChoice::Exit {
            self.request_exit();
        }
        true
    }

    pub fn request_exit(&mut self) {
        self.pending_exit_confirm = None;
        self.should_exit = true;
        if matches!(
            self.flow.current_status(),
            crate::state::step::StepStatus::Active | crate::state::step::StepStatus::Running
        ) {
            self.flow.cancel_current();
        }
        crate::task::engine::cancel_interval_tasks(self);
        self.cancel_all_running_tasks();
        self.runtime.queued_task_requests.clear();
    }

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

    fn active_blocking_overlay_info(&self) -> Option<(NodeId, FocusMode)> {
        let entry = self.ui.overlays.active_blocking()?;
        Some((entry.id.clone(), entry.focus_mode))
    }

    fn overlay_has_persistent_children(&self, overlay_id: &str) -> bool {
        find_overlay(self.flow.current_step().nodes.as_slice(), overlay_id)
            .and_then(Node::persistent_children)
            .is_some()
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

    pub fn validation_state(&self) -> &ValidationState {
        &self.runtime.validation
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.runtime.validation.visible_error(id)
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.runtime.validation.is_hidden_invalid(id)
    }

    pub fn clear_step_errors(&mut self) {
        self.runtime.validation.clear_step_errors();
        self.runtime.validation.clear_step_warnings();
        self.runtime.validation.reset_warnings_acknowledged();
    }

    pub(super) fn refresh_validation_after_change(&mut self) {
        self.validate_focused_live();
        self.runtime.validation.reset_warnings_acknowledged();
    }

    pub fn current_step_errors(&self) -> &[String] {
        self.runtime.validation.step_errors()
    }

    pub fn current_step_warnings(&self) -> &[String] {
        self.runtime.validation.step_warnings()
    }

    fn prepare_current_step_for_preview(&mut self) {
        self.reconcile_current_step_visibility();
        self.ui.overlays.clear();
        self.hydrate_current_step_from_store();
        self.rebuild_focus();
    }
}

mod adapters;
mod effects;
mod flow;
mod input;
mod state;
mod validation_runtime;
mod value_sync;

fn collect_inline_tasks(flow: &Flow) -> (Vec<TaskSpec>, Vec<TaskSubscription>) {
    let mut specs = Vec::<TaskSpec>::new();
    let mut subscriptions = Vec::<TaskSubscription>::new();

    for step in flow.steps() {
        walk_nodes(
            step.nodes.as_slice(),
            NodeWalkScope::Recursive,
            &mut |node| {
                specs.extend(node.task_specs());
                subscriptions.extend(node.task_subscriptions());
            },
        );
    }

    (specs, subscriptions)
}
