use crate::core::form_engine::FormEngine;
use crate::core::layer::{ActiveLayer, Layer};
use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;

pub struct LayerManager {
    active: Option<ActiveLayer>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            active: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&ActiveLayer> {
        self.active.as_ref()
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

        let saved_focus_id = engine.focused_id().cloned();

        for (id, node) in layer.nodes() {
            registry.insert(id, node);
        }

        let input_ids: Vec<NodeId> = registry.input_ids_for_step_owned(layer.node_ids());

        engine.reset_with_ids(input_ids, registry);

        self.active = Some(ActiveLayer::new(layer, saved_focus_id));
    }

    pub fn close(
        &mut self,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
        step_input_ids: Vec<NodeId>,
    ) -> bool {
        let Some(active) = self.active.take() else {
            return false;
        };

        for id in active.layer.node_ids() {
            registry.remove(id);
        }

        engine.reset_with_ids(step_input_ids.clone(), registry);

        if let Some(saved_id) = active.saved_focus_id {
            if let Some(index) = step_input_ids.iter().position(|id| id == &saved_id) {
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
        step_input_ids: Vec<NodeId>,
    ) {
        if self.active.is_some() {
            self.close(registry, engine, step_input_ids);
        } else {
            self.open(layer_fn(), registry, engine);
        }
    }
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}
