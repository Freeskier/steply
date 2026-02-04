use crate::core::binding::BindTarget;
use crate::core::event_queue::AppEvent;
use crate::core::form_engine::FormEngine;
use crate::core::layer::{ActiveLayer, Layer};
use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;

pub struct LayerManager {
    active: Option<ActiveLayer>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&ActiveLayer> {
        self.active.as_ref()
    }

    pub fn active_mut(&mut self) -> Option<&mut ActiveLayer> {
        self.active.as_mut()
    }

    pub fn open(
        &mut self,
        mut layer: Box<dyn Layer>,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
    ) {
        if self.active.is_some() {
            return;
        }

        let saved_focus_id = engine.focused_node_id().cloned();
        if layer.bind_target().is_none() {
            let target = saved_focus_id
                .as_ref()
                .and_then(|id| bind_target_from_id(registry, id));
            if let Some(target) = target {
                layer.set_bind_target(Some(target));
            }
        }

        for (id, node) in layer.nodes() {
            registry.insert(id, node);
        }

        let node_ids: Vec<NodeId> = layer.node_ids().to_vec();
        engine.reset_with_nodes(node_ids, registry);

        self.active = Some(ActiveLayer::new(layer, saved_focus_id));
    }

    pub fn close(
        &mut self,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
        step_node_ids: Vec<NodeId>,
        emit: &mut dyn FnMut(AppEvent),
    ) -> bool {
        let Some(mut active) = self.active.take() else {
            return false;
        };

        active.layer.emit_close_events(registry, emit);

        for id in active.layer.node_ids() {
            registry.remove(id);
        }

        engine.reset_with_nodes(step_node_ids.clone(), registry);

        if let Some(saved_id) = active.saved_focus_id {
            if let Some(index) = engine.find_index_by_id(&saved_id) {
                let mut events = Vec::new();
                engine.set_focus(registry, Some(index), &mut events);
            }
        }

        true
    }

    pub fn toggle(
        &mut self,
        layer_fn: impl FnOnce() -> Box<dyn Layer>,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
        step_node_ids: Vec<NodeId>,
    ) {
        if self.active.is_some() {
            let mut emit = |_| {};
            self.close(registry, engine, step_node_ids, &mut emit);
        } else {
            self.open(layer_fn(), registry, engine);
        }
    }
}

fn bind_target_from_id(registry: &NodeRegistry, id: &str) -> Option<BindTarget> {
    if registry.get_input(id).is_some() {
        return Some(BindTarget::Input(id.to_string()));
    }
    if registry.get_component(id).is_some() {
        return Some(BindTarget::Component(id.to_string()));
    }
    None
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}
