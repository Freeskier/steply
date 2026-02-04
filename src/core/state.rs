use crate::core::flow::Flow;
use crate::core::form_engine::FormEngine;

pub struct AppState {
    pub flow: Flow,
    pub engine: FormEngine,
    pub should_exit: bool,
}

impl AppState {
    pub fn new(mut flow: Flow) -> Self {
        let node_ids: Vec<_> = flow.current_step().node_ids.clone();
        let input_ids = flow.registry().input_ids_for_step_owned(&node_ids);

        let engine = FormEngine::from_input_ids(input_ids, flow.registry_mut());

        Self {
            flow,
            engine,
            should_exit: false,
        }
    }

    pub fn reset_engine_for_current_step(&mut self) {
        let node_ids = self.flow.current_step().node_ids.clone();
        let input_ids = self.flow.registry().input_ids_for_step_owned(&node_ids);
        let registry = self.flow.registry_mut();
        self.engine.reset_with_ids(input_ids, registry);
    }
}
