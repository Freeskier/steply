use crate::core::flow::Flow;
use crate::core::form_engine::FormEngine;

pub struct AppState {
    pub flow: Flow,
    pub engine: FormEngine,
    pub should_exit: bool,
}

impl AppState {
    pub fn new(mut flow: Flow) -> Self {
        let nodes = flow.current_step_mut().nodes.as_mut_slice();
        let engine = FormEngine::from_nodes(nodes);

        Self {
            flow,
            engine,
            should_exit: false,
        }
    }

    pub fn reset_engine_for_current_step(&mut self) {
        let nodes = self.flow.current_step_mut().nodes.as_mut_slice();
        self.engine.reset_with_nodes(nodes);
    }
}
