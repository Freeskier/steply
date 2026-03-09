use std::collections::HashMap;

use crate::state::flow::Flow;
use crate::task::{
    TaskId, TaskSpec, TaskSubscription, collect_inline_tasks_from_flow, validate_task_id_collisions,
};

use super::state::{DataState, RuntimeState, ViewState};
use super::{AppState, AppStateInitError};

impl AppState {
    pub fn new(flow: Flow) -> Result<Self, AppStateInitError> {
        Self::with_tasks(flow, Vec::new(), Vec::new())
    }

    pub fn with_tasks(
        flow: Flow,
        task_specs: Vec<TaskSpec>,
        task_subscriptions: Vec<TaskSubscription>,
    ) -> Result<Self, AppStateInitError> {
        let (inline_specs, inline_subscriptions) = collect_inline_tasks_from_flow(&flow);
        validate_task_id_collisions(&task_specs, &inline_specs)?;
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
        Ok(state)
    }

    pub(super) fn prepare_current_step_for_preview(&mut self) {
        self.reconcile_current_step_visibility();
        self.ui.overlays.clear();
        self.hydrate_current_step_from_store();
        self.rebuild_focus();
    }
}
