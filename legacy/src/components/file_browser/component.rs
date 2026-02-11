use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::inputs::Input;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderOutput};
use std::sync::{Arc, Mutex};

use super::shared_state::SharedFileBrowserState;
use super::{EntryFilter, FileBrowserState};

pub struct FileBrowserComponent {
    base: ComponentBase,
    state: SharedFileBrowserState,
}

impl FileBrowserComponent {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let state = Arc::new(Mutex::new(FileBrowserState::new(id.clone())));
        Self {
            base: ComponentBase::new(id),
            state: SharedFileBrowserState::new(state),
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

    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.state
            .with_mut(|state| state.set_max_visible(max_visible));
    }

    pub fn with_recursive_search(self, recursive: bool) -> Self {
        self.state
            .with_mut(|state| state.set_recursive_search(recursive));
        self
    }

    pub fn with_entry_filter(self, filter: EntryFilter) -> Self {
        self.state.with_mut(|state| state.set_entry_filter(filter));
        self
    }

    pub fn set_entry_filter(&mut self, filter: EntryFilter) {
        self.state.with_mut(|state| state.set_entry_filter(filter));
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

    pub fn set_extension_filter<I, S>(&mut self, exts: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.state
            .with_mut(|state| state.set_extension_filter(exts));
    }

    pub fn clear_extension_filter(&mut self) {
        self.state
            .with_mut(FileBrowserState::clear_extension_filter);
    }

    pub fn with_relative_paths(self, show_relative: bool) -> Self {
        self.state
            .with_mut(|state| state.set_relative_paths(show_relative));
        self
    }

    pub fn set_relative_paths(&mut self, show_relative: bool) {
        self.state
            .with_mut(|state| state.set_relative_paths(show_relative));
    }

    pub fn with_show_hidden(self, show_hidden: bool) -> Self {
        self.state
            .with_mut(|state| state.set_show_hidden(show_hidden));
        self
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) {
        self.state
            .with_mut(|state| state.set_show_hidden(show_hidden));
    }

    pub fn set_current_dir(&mut self, dir: impl Into<std::path::PathBuf>) {
        self.state.with_mut(|state| state.set_current_dir(dir));
    }

    pub fn with_show_info(self, show_info: bool) -> Self {
        self.state.with_mut(|state| state.set_show_info(show_info));
        self
    }
}

impl Component for FileBrowserComponent {
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
        self.state.with_mut(|state| {
            let mut output = state.render_input_line(ctx, self.base.focused);
            output.append(state.render_list_lines(ctx, self.base.focused));
            output
        })
    }

    fn value(&self) -> Option<Value> {
        self.state.with(FileBrowserState::selected_value)
    }

    fn set_value(&mut self, value: Value) {
        self.state.with_mut(|state| state.set_value(value));
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        self.state
            .with_mut(|state| state.handle_combined_key(code, modifiers))
    }

    fn poll(&mut self) -> bool {
        self.state.with_mut(FileBrowserState::poll)
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.state.with_mut(|state| {
            state.input_mut().set_focused(focused);
            state.select_mut().set_focused(focused);
        });
    }

    fn delete_word(&mut self) -> ComponentResponse {
        self.state.with_mut(FileBrowserState::delete_word)
    }

    fn delete_word_forward(&mut self) -> ComponentResponse {
        self.state.with_mut(FileBrowserState::delete_word_forward)
    }
}
