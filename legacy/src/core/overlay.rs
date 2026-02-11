use crate::core::binding::{BindTarget, ValueSource};
use crate::core::component::ComponentResponse;
use crate::core::event_queue::AppEvent;
use crate::core::layer::Layer;
use crate::core::layer::LayerFocusMode;
use crate::core::node::{Node, first_input, first_input_mut};
use crate::core::value::Value;
use crate::terminal::KeyEvent;
use crate::text_input::TextInput;

pub struct OverlayState {
    id: String,
    label: String,
    hint: Option<String>,
    nodes: Vec<Node>,
    bind_target: Option<BindTarget>,
    focus_mode: LayerFocusMode,
    key_handler: Option<
        Box<dyn FnMut(&mut OverlayState, KeyEvent, &mut dyn FnMut(AppEvent)) -> bool + Send>,
    >,
}

impl OverlayState {
    pub fn new(id: impl Into<String>, label: impl Into<String>, nodes: Vec<Node>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            hint: None,
            nodes,
            bind_target: None,
            focus_mode: LayerFocusMode::Modal,
            key_handler: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_bind_target(mut self, target: BindTarget) -> Self {
        self.bind_target = Some(target);
        self
    }

    pub fn with_focus_mode(mut self, focus_mode: LayerFocusMode) -> Self {
        self.focus_mode = focus_mode;
        self
    }

    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut OverlayState, KeyEvent, &mut dyn FnMut(AppEvent)) -> bool + Send + 'static,
    {
        self.key_handler = Some(Box::new(handler));
        self
    }

    pub fn demo() -> Self {
        let input_id = "overlay_query".to_string();
        let nodes = vec![Node::input(TextInput::new(&input_id, "Search"))];
        Self::new("overlay_demo", "Overlay demo: type, Esc to close", nodes)
    }

    pub fn emit_value(&self, value: Value, emit: &mut dyn FnMut(AppEvent)) {
        let Some(target) = self.bind_target.clone() else {
            return;
        };
        emit(AppEvent::ValueProduced {
            source: ValueSource::Layer(self.id.clone()),
            target,
            value,
        });
    }

    pub fn emit_response(&self, response: ComponentResponse, emit: &mut dyn FnMut(AppEvent)) {
        if let Some(value) = response.produced {
            self.emit_value(value, emit);
        }
    }
}

impl Layer for OverlayState {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    fn nodes_mut(&mut self) -> &mut [Node] {
        &mut self.nodes
    }

    fn focus_mode(&self) -> LayerFocusMode {
        self.focus_mode
    }

    fn bind_target(&self) -> Option<BindTarget> {
        self.bind_target.clone()
    }

    fn set_bind_target(&mut self, target: Option<BindTarget>) {
        self.bind_target = target;
    }

    fn set_value(&mut self, value: Value) {
        let Some(input) = first_input_mut(&mut self.nodes) else {
            return;
        };
        input.set_value_typed(value);
    }

    fn emit_close_events(&mut self, emit: &mut dyn FnMut(AppEvent)) {
        let Some(input) = first_input(&self.nodes) else {
            return;
        };

        let value = input.value_typed();
        if value.is_empty() {
            return;
        }

        let Some(target) = self.bind_target.clone() else {
            return;
        };

        emit(AppEvent::ValueProduced {
            source: ValueSource::Layer(self.id.clone()),
            target,
            value,
        });
    }

    fn handle_key(&mut self, key: KeyEvent, emit: &mut dyn FnMut(AppEvent)) -> bool {
        let Some(mut handler) = self.key_handler.take() else {
            return false;
        };
        let handled = handler(self, key, emit);
        self.key_handler = Some(handler);
        handled
    }
}
