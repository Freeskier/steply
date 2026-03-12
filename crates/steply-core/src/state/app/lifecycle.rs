use crate::state::change::collect_store_ownership;
use crate::state::flow::Flow;
use crate::task::{TaskSpec, collect_inline_tasks_from_flow, validate_task_id_collisions};

use super::state::{DataState, RuntimeState, ViewState};
use super::{AppState, AppStateInitError};

impl AppState {
    pub fn new(flow: Flow) -> Result<Self, AppStateInitError> {
        Self::with_tasks(flow, Vec::new())
    }

    pub fn with_tasks(flow: Flow, task_specs: Vec<TaskSpec>) -> Result<Self, AppStateInitError> {
        let inline_specs = collect_inline_tasks_from_flow(&flow);
        validate_task_id_collisions(&task_specs, &inline_specs)?;
        let mut specs = inline_specs;
        specs.extend(task_specs);

        let mut state = Self {
            flow,
            ui: ViewState::default(),
            data: DataState::default(),
            runtime: RuntimeState::with_tasks(specs),
            scratch_nodes: Vec::new(),
            should_exit: false,
            pending_back_confirm: None,
            pending_exit_confirm: None,
        };
        state.runtime.store_ownership =
            collect_store_ownership(&state.flow, state.runtime.task_specs.values().cloned());
        if state.flow.is_empty() {
            state.should_exit = true;
        } else {
            state.reconcile_current_step_visibility();
            state.refresh_current_step_bindings();
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
        self.refresh_current_step_bindings();
        self.rebuild_focus();
    }
}
