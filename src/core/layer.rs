use crate::core::binding::BindTarget;
use crate::core::event_queue::AppEvent;
use crate::core::node::Node;
use crate::core::node::NodeId;
use crate::core::value::Value;

pub trait Layer {
    fn id(&self) -> &str;

    fn label(&self) -> &str;

    fn hint(&self) -> Option<&str>;

    fn nodes(&self) -> &[Node];

    fn nodes_mut(&mut self) -> &mut [Node];

    fn bind_target(&self) -> Option<BindTarget> {
        None
    }

    fn set_bind_target(&mut self, _target: Option<BindTarget>) {}

    fn set_value(&mut self, _value: Value) {}

    fn emit_close_events(&mut self, _emit: &mut dyn FnMut(AppEvent)) {}
}

pub struct ActiveLayer {
    pub layer: Box<dyn Layer>,
    pub saved_focus_id: Option<NodeId>,
}

impl ActiveLayer {
    pub fn new(layer: Box<dyn Layer>, saved_focus_id: Option<NodeId>) -> Self {
        Self {
            layer,
            saved_focus_id,
        }
    }

    pub fn label(&self) -> &str {
        self.layer.label()
    }

    pub fn hint(&self) -> Option<&str> {
        self.layer.hint()
    }

    pub fn nodes(&self) -> &[Node] {
        self.layer.nodes()
    }

    pub fn nodes_mut(&mut self) -> &mut [Node] {
        self.layer.nodes_mut()
    }
}
