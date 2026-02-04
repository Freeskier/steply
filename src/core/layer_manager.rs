use crate::core::form_engine::FormEngine;
use crate::core::layer::{ActiveLayer, Layer};
use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;

/// Manages UI layers (overlays, modals, etc.) on top of the main step.
pub struct LayerManager {
    active: Option<ActiveLayer>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            active: None,
        }
    }

    /// Returns true if a layer is currently active
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Returns the active layer if any
    pub fn active(&self) -> Option<&ActiveLayer> {
        self.active.as_ref()
    }

    /// Open a layer, saving current focus and registering layer nodes
    pub fn open(
        &mut self,
        mut layer: Box<dyn Layer>,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
    ) {
        if self.active.is_some() {
            return;
        }

        // Save current focus
        let saved_focus_id = engine.focused_id().cloned();

        // Register layer nodes
        for (id, node) in layer.nodes() {
            registry.insert(id, node);
        }

        // Get input IDs from layer nodes
        let input_ids: Vec<NodeId> = registry.input_ids_for_step_owned(layer.node_ids());

        // Reset engine to layer inputs
        engine.reset_with_ids(input_ids, registry);

        self.active = Some(ActiveLayer::new(layer, saved_focus_id));
    }

    /// Close the active layer, restoring focus and removing layer nodes
    pub fn close(
        &mut self,
        registry: &mut NodeRegistry,
        engine: &mut FormEngine,
        step_input_ids: Vec<NodeId>,
    ) -> bool {
        let Some(active) = self.active.take() else {
            return false;
        };

        // Remove layer nodes from registry
        for id in active.layer.node_ids() {
            registry.remove(id);
        }

        // Reset engine to step inputs
        engine.reset_with_ids(step_input_ids.clone(), registry);

        // Restore saved focus if still valid
        if let Some(saved_id) = active.saved_focus_id {
            if let Some(index) = step_input_ids.iter().position(|id| id == &saved_id) {
                let mut events = Vec::new();
                engine.set_focus(registry, Some(index), &mut events);
            }
        }

        true
    }

    /// Toggle layer - close if open, open if closed
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
