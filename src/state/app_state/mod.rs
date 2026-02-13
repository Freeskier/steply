use crate::core::NodeId;
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::state::store::ValueStore;
use crate::state::validation::ValidationState;
use crate::task::{
    TaskCancelToken, TaskId, TaskInvocation, TaskRequest, TaskRunState, TaskSpec, TaskSubscription,
};
use crate::widgets::node::{Node, NodeWalkScope, find_overlay, find_overlay_mut, walk_nodes};
use crate::widgets::node_index::NodeIndex;
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayPlacement};
use focus_engine::FocusEngine;
use overlay_engine::OverlayEngine;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct CompletionSession {
    pub owner_id: NodeId,
    pub matches: Vec<String>,
    pub index: usize,
    pub start: usize,
}

#[derive(Default)]
struct UiState {
    overlays: OverlayEngine,
    focus: FocusEngine,
    active_node_index: NodeIndex,
    completion_session: Option<CompletionSession>,
}

#[derive(Default)]
struct DataState {
    store: ValueStore,
}

#[derive(Default)]
struct RuntimeState {
    validation: ValidationState,
    pending_scheduler: Vec<SchedulerCommand>,
    pending_task_invocations: Vec<TaskInvocation>,
    queued_task_requests: HashMap<TaskId, Vec<TaskRequest>>,
    running_task_cancellations: HashMap<TaskId, Vec<(u64, TaskCancelToken)>>,
    task_runs: HashMap<TaskId, TaskRunState>,
    task_specs: HashMap<TaskId, TaskSpec>,
    task_subscriptions: Vec<TaskSubscription>,
}

pub struct AppState {
    flow: Flow,
    ui: UiState,
    data: DataState,
    runtime: RuntimeState,
    scratch_nodes: Vec<Node>,
    should_exit: bool,
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
        let mut spec_map = HashMap::<TaskId, TaskSpec>::new();
        for spec in task_specs {
            spec_map.insert(spec.id.clone(), spec);
        }

        let mut state = Self {
            flow,
            ui: UiState::default(),
            data: DataState::default(),
            runtime: RuntimeState {
                validation: ValidationState::default(),
                pending_scheduler: Vec::new(),
                pending_task_invocations: Vec::new(),
                queued_task_requests: HashMap::new(),
                running_task_cancellations: HashMap::new(),
                task_runs: HashMap::new(),
                task_specs: spec_map,
                task_subscriptions,
            },
            scratch_nodes: Vec::new(),
            should_exit: false,
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

    pub fn current_hint(&self) -> Option<&str> {
        if self.flow.is_empty() {
            return None;
        }
        self.flow.current_step().hint.as_deref()
    }

    pub fn focused_id(&self) -> Option<&str> {
        self.ui.focus.current_id()
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn request_exit(&mut self) {
        self.should_exit = true;
        self.cancel_interval_tasks();
        self.cancel_all_running_tasks();
        self.runtime.queued_task_requests.clear();
    }

    pub fn active_nodes(&self) -> &[Node] {
        if self.flow.is_empty() {
            return &[];
        }
        let step_nodes = self.flow.current_step().nodes.as_slice();
        if let Some(entry) = self.ui.overlays.active_blocking()
            && let Some(overlay) = find_overlay(step_nodes, entry.id.as_str())
        {
            if overlay.focus_mode() == FocusMode::Group {
                return step_nodes;
            }
            if let Some(children) = overlay.persistent_children() {
                return children;
            }
        }
        step_nodes
    }

    pub fn active_nodes_mut(&mut self) -> &mut [Node] {
        if self.flow.is_empty() {
            return self.scratch_nodes.as_mut_slice();
        }
        let active_blocking = self.ui.overlays.active_blocking().cloned();
        if let Some(active_blocking) = active_blocking {
            if active_blocking.focus_mode == FocusMode::Group {
                return self.flow.current_step_mut().nodes.as_mut_slice();
            }

            let has_overlay_children = find_overlay(
                self.flow.current_step().nodes.as_slice(),
                active_blocking.id.as_str(),
            )
            .and_then(Node::persistent_children)
            .is_some();

            if has_overlay_children {
                let step_nodes = self.flow.current_step_mut().nodes.as_mut_slice();
                if let Some(overlay) = find_overlay_mut(step_nodes, active_blocking.id.as_str())
                    && let Some(children) = overlay.persistent_children_mut()
                {
                    return children;
                }

                return self.scratch_nodes.as_mut_slice();
            }

            self.ui.overlays.clear();
            return self.flow.current_step_mut().nodes.as_mut_slice();
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

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.runtime.validation.visible_error(id)
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.runtime.validation.is_hidden_invalid(id)
    }

    pub fn clear_step_errors(&mut self) {
        self.runtime.validation.clear_step_errors();
    }

    pub fn current_step_errors(&self) -> &[String] {
        self.runtime.validation.step_errors()
    }
}

mod completion;
mod focus_engine;
mod navigation;
mod overlay_engine;
mod overlay_runtime;
mod task_runtime;
mod validation_runtime;
mod value_sync;
