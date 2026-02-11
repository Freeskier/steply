use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::inputs::Input;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderOutput};
use std::sync::{Arc, Mutex};

use super::shared_state::SharedFileBrowserState;
use super::state::FileBrowserState;

pub struct FileBrowserInputComponent {
    base: ComponentBase,
    state: SharedFileBrowserState,
}

impl FileBrowserInputComponent {
    pub fn from_state(state: Arc<Mutex<FileBrowserState>>) -> Self {
        let shared = SharedFileBrowserState::new(state);
        let id = shared.with(|guard| guard.input_id().to_string());
        Self {
            base: ComponentBase::new(id),
            state: shared,
        }
    }

    pub fn with_label(self, label: impl Into<String>) -> Self {
        self.state.with_mut(|state| state.set_label(label));
        self
    }

    pub fn with_placeholder(self, placeholder: impl Into<String>) -> Self {
        self.state
            .with_mut(|state| state.set_placeholder(placeholder));
        self
    }

    pub fn with_max_visible(self, max_visible: usize) -> Self {
        self.state
            .with_mut(|state| state.set_max_visible(max_visible));
        self
    }

    pub fn with_recursive_search(self, recursive: bool) -> Self {
        self.state
            .with_mut(|state| state.set_recursive_search(recursive));
        self
    }

    pub fn with_entry_filter(self, filter: super::EntryFilter) -> Self {
        self.state.with_mut(|state| state.set_entry_filter(filter));
        self
    }

    pub fn with_extension_filter<I, S>(self, exts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.state
            .with_mut(|state| state.set_extension_filter(exts));
        self
    }

    pub fn with_relative_paths(self, show_relative: bool) -> Self {
        self.state
            .with_mut(|state| state.set_relative_paths(show_relative));
        self
    }

    pub fn with_show_hidden(self, show_hidden: bool) -> Self {
        self.state
            .with_mut(|state| state.set_show_hidden(show_hidden));
        self
    }

    pub fn with_show_info(self, show_info: bool) -> Self {
        self.state.with_mut(|state| state.set_show_info(show_info));
        self
    }

    pub fn state(&self) -> Arc<Mutex<FileBrowserState>> {
        self.state.arc()
    }
}

impl Component for FileBrowserInputComponent {
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
            .with(|state| state.render_input_line(ctx, self.base.focused))
    }

    fn value(&self) -> Option<Value> {
        self.state.with(FileBrowserState::selected_value)
    }

    fn set_value(&mut self, value: Value) {
        self.state.with_mut(|state| state.set_value(value));
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        self.state
            .with_mut(|state| state.handle_input_key(code, modifiers))
    }

    fn poll(&mut self) -> bool {
        self.state.with_mut(FileBrowserState::poll)
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.state
            .with_mut(|state| state.input_mut().set_focused(focused));
    }

    fn delete_word(&mut self) -> ComponentResponse {
        self.state.with_mut(FileBrowserState::delete_word)
    }

    fn delete_word_forward(&mut self) -> ComponentResponse {
        self.state.with_mut(FileBrowserState::delete_word_forward)
    }
}
