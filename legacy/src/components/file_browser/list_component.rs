use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderOutput};
use std::sync::{Arc, Mutex};

use super::shared_state::SharedFileBrowserState;
use super::state::FileBrowserState;

pub struct FileBrowserListComponent {
    base: ComponentBase,
    state: SharedFileBrowserState,
}

impl FileBrowserListComponent {
    pub fn from_state(state: Arc<Mutex<FileBrowserState>>) -> Self {
        let shared = SharedFileBrowserState::new(state);
        let id = shared.with(|guard| guard.list_id().to_string());
        Self {
            base: ComponentBase::new(id),
            state: shared,
        }
    }

    pub fn state(&self) -> Arc<Mutex<FileBrowserState>> {
        self.state.arc()
    }
}

impl Component for FileBrowserListComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn render(&self, ctx: &RenderContext) -> RenderOutput {
        self.state
            .with_mut(|state| state.render_list_lines(ctx, true))
    }

    fn value(&self) -> Option<Value> {
        self.state.with(FileBrowserState::selected_value)
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        self.state
            .with_mut(|state| state.handle_list_key(code, modifiers))
    }

    fn poll(&mut self) -> bool {
        self.state.with_mut(FileBrowserState::poll)
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
    }
}
