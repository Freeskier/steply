use crate::core::flow::Flow;
use crate::core::form_engine::FormEngine;
use crate::core::view_state::ViewState;

pub struct AppState {
    pub flow: Flow,
    pub engine: FormEngine,
    pub view: ViewState,
    pub should_exit: bool,
}

impl AppState {
    pub fn new(flow: Flow) -> Self {
        let mut flow = flow;
        let engine = {
            let step = flow.current_step_mut();
            FormEngine::new(step)
        };
        Self {
            flow,
            engine,
            view: ViewState::new(),
            should_exit: false,
        }
    }
}
