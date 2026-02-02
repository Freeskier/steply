use crate::core::form_engine::FormEngine;
use crate::core::step::Step;
use crate::core::view_state::ViewState;

pub struct AppState {
    pub engine: FormEngine,
    pub view: ViewState,
    pub should_exit: bool,
}

impl AppState {
    pub fn new(step: Step) -> Self {
        Self {
            engine: FormEngine::new(step),
            view: ViewState::new(),
            should_exit: false,
        }
    }
}
