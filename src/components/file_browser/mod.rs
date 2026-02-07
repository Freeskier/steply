pub mod cache;
pub mod component;
pub mod input_component;
pub mod list_component;
pub mod model;
pub mod parser;
pub mod scanner;
pub mod search;
pub mod search_cache;
pub mod search_state;
pub mod state;

use std::sync::{Arc, Mutex};

pub use component::FileBrowserComponent;
pub use input_component::FileBrowserInputComponent;
pub use list_component::FileBrowserListComponent;
pub use model::EntryFilter;
pub use state::FileBrowserState;

pub struct FileBrowserBundle {
    pub state: Arc<Mutex<FileBrowserState>>,
    pub input: FileBrowserInputComponent,
    pub list: FileBrowserListComponent,
}

impl FileBrowserBundle {
    pub fn new(id: impl Into<String>) -> Self {
        let state = Arc::new(Mutex::new(FileBrowserState::new(id)));
        let input = FileBrowserInputComponent::from_state(state.clone());
        let list = FileBrowserListComponent::from_state(state.clone());
        Self { state, input, list }
    }
}

pub fn overlay_for_list(
    id: impl Into<String>,
    label: impl Into<String>,
    state: Arc<Mutex<FileBrowserState>>,
) -> crate::core::overlay::OverlayState {
    use crate::core::component::Component;
    use crate::core::layer::Layer;
    use crate::core::layer::LayerFocusMode;
    use crate::core::node::{Node, find_component_mut};
    use crate::terminal::KeyCode;

    let list_component = FileBrowserListComponent::from_state(state);
    let list_id = list_component.id().to_string();
    let nodes = vec![Node::component(list_component)];

    crate::core::overlay::OverlayState::new(id, label, nodes)
        .with_focus_mode(LayerFocusMode::Shared)
        .with_key_handler(move |overlay, key, emit| {
            let wants_key = matches!(
                key.code,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Enter
                    | KeyCode::Char(' ')
            );
            if !wants_key {
                return false;
            }
            let Some(component) = find_component_mut(overlay.nodes_mut(), &list_id) else {
                return false;
            };
            let response = component.handle_key(key.code, key.modifiers);
            let handled = response.handled;
            overlay.emit_response(response, emit);
            handled
        })
}
