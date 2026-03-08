use std::collections::HashMap;

use crate::state::flow::Flow;
use crate::task::{TaskId, TaskSpec, TaskSubscription};
use crate::widgets::node::{NodeWalkScope, walk_nodes};

use super::AppState;
use super::state::{DataState, RuntimeState, ViewState};

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

    pub(super) fn prepare_current_step_for_preview(&mut self) {
        self.reconcile_current_step_visibility();
        self.ui.overlays.clear();
        self.hydrate_current_step_from_store();
        self.rebuild_focus();
    }
}

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
