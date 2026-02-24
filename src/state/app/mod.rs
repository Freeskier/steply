use crate::core::NodeId;
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::flow::Flow;
use crate::state::focus::FocusState;
use crate::state::overlay::OverlayState;
use crate::state::step::Step;
use crate::state::store::ValueStore;
use crate::state::validation::ValidationState;
use crate::task::{
    TaskCancelToken, TaskId, TaskInvocation, TaskRequest, TaskRunState, TaskSpec, TaskSubscription,
};
use crate::widgets::node::{Node, NodeWalkScope, find_overlay, find_overlay_mut, walk_nodes};
use crate::widgets::node_index::NodeIndex;
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};
use completion::CompletionSession;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
struct ViewState {
    overlays: OverlayState,
    focus: FocusState,
    active_node_index: NodeIndex,
    completion_session: Option<CompletionSession>,
    completion_tab_suppressed_for: Option<NodeId>,
    hints_visible: bool,
}

#[derive(Default)]
struct DataState {
    store: ValueStore,
}

#[derive(Clone)]
struct RunningTaskHandle {
    run_id: u64,
    cancel_token: TaskCancelToken,
    origin_step_id: Option<String>,
}

#[derive(Default)]
struct RuntimeState {
    validation: ValidationState,
    pending_scheduler: Vec<SchedulerCommand>,
    pending_task_invocations: Vec<TaskInvocation>,
    queued_task_requests: HashMap<TaskId, VecDeque<TaskRequest>>,
    running_task_cancellations: HashMap<TaskId, Vec<RunningTaskHandle>>,
    task_runs: HashMap<TaskId, TaskRunState>,
    task_specs: HashMap<TaskId, TaskSpec>,
    task_subscriptions: Vec<TaskSubscription>,
}

pub struct AppState {
    flow: Flow,
    ui: ViewState,
    data: DataState,
    runtime: RuntimeState,
    scratch_nodes: Vec<Node>,
    should_exit: bool,
    pending_back_confirm: Option<String>,
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
            runtime: RuntimeState {
                validation: ValidationState::default(),
                pending_scheduler: Vec::new(),
                pending_task_invocations: Vec::new(),
                queued_task_requests: HashMap::new(),
                running_task_cancellations: HashMap::new(),
                task_runs: HashMap::new(),
                task_specs: spec_map,
                task_subscriptions: subscriptions,
            },
            scratch_nodes: Vec::new(),
            should_exit: false,
            pending_back_confirm: None,
        };
        if state.flow.is_empty() {
            state.should_exit = true;
        } else {
            state.rebuild_focus();
            state.trigger_flow_start_tasks();
            let current_step_id = state.current_step_id().to_string();
            state.trigger_step_enter_tasks(current_step_id.as_str());
            state.bootstrap_interval_tasks();
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

    pub fn steps(&self) -> &[Step] {
        self.flow.steps()
    }

    pub fn step_status_at(&self, index: usize) -> crate::state::step::StepStatus {
        self.flow.status_at(index)
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

    pub fn request_exit(&mut self) {
        self.should_exit = true;
        if matches!(
            self.flow.current_status(),
            crate::state::step::StepStatus::Active | crate::state::step::StepStatus::Running
        ) {
            self.flow.cancel_current();
        }
        self.cancel_interval_tasks();
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
            NodeWalkScope::Persistent,
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
        self.clear_step_errors();
    }

    pub fn current_step_errors(&self) -> &[String] {
        self.runtime.validation.step_errors()
    }

    pub fn current_step_warnings(&self) -> &[String] {
        self.runtime.validation.step_warnings()
    }
}

mod completion;
mod completion_engine;
mod completion_session;
mod effect_dispatcher;
mod input_dispatch;
mod navigation;
mod overlay_runtime;
mod step_flow;
mod task_engine;
mod task_runtime;
mod validation_runtime;
mod value_sync;

fn collect_inline_tasks(flow: &Flow) -> (Vec<TaskSpec>, Vec<TaskSubscription>) {
    let mut specs = Vec::<TaskSpec>::new();
    let mut subscriptions = Vec::<TaskSubscription>::new();

    for step in flow.steps() {
        walk_nodes(
            step.nodes.as_slice(),
            NodeWalkScope::Persistent,
            &mut |node| {
                specs.extend(node.task_specs());
                subscriptions.extend(node.task_subscriptions());
            },
        );
    }

    (specs, subscriptions)
}
