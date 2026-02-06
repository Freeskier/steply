use crate::core::component::{Component, ComponentBase, EventContext, FocusMode};
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine};
use std::sync::{Arc, Mutex};

use super::state::FileBrowserState;

pub struct FileBrowserListComponent {
    base: ComponentBase,
    state: Arc<Mutex<FileBrowserState>>,
}

impl FileBrowserListComponent {
    pub fn from_state(state: Arc<Mutex<FileBrowserState>>) -> Self {
        let id = state.lock().unwrap().list_id().to_string();
        Self {
            base: ComponentBase::new(id),
            state,
        }
    }

    pub fn state(&self) -> Arc<Mutex<FileBrowserState>> {
        Arc::clone(&self.state)
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

    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut state = self.state.lock().unwrap();
        state.render_list_lines(ctx, true)
    }

    fn value(&self) -> Option<Value> {
        self.state.lock().unwrap().selected_value()
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool {
        self.state
            .lock()
            .unwrap()
            .handle_list_key(code, modifiers, ctx)
    }

    fn poll(&mut self) -> bool {
        self.state.lock().unwrap().poll()
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
    }
}
