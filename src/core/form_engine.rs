use crate::core::form_event::FormEvent;
use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;
use crate::core::step::Step;
use crate::core::validation;
use crate::inputs::{Input, InputCaps, InputError, KeyResult};
use crate::terminal::KeyEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusTarget {
    Input(NodeId),
    Component(NodeId),
}

pub struct FormEngine {
    input_ids: Vec<NodeId>,
    focus_targets: Vec<FocusTarget>,
    focus_index: Option<usize>,
}

impl FormEngine {
    pub fn new(step: &Step, registry: &mut NodeRegistry) -> Self {
        let node_ids = step.node_ids.clone();
        Self::from_node_ids(node_ids, registry)
    }

    pub fn from_node_ids(node_ids: Vec<NodeId>, registry: &mut NodeRegistry) -> Self {
        let input_ids = registry.input_ids_for_step_owned(&node_ids);
        let focus_targets = Self::focus_targets_for_ids(&node_ids, registry);
        let mut engine = Self {
            input_ids,
            focus_targets,
            focus_index: None,
        };

        if !engine.focus_targets.is_empty() {
            engine.set_focus_internal(registry, Some(0));
        }

        engine
    }

    pub fn reset(&mut self, step: &Step, registry: &mut NodeRegistry) {
        let node_ids = step.node_ids.clone();
        self.reset_with_nodes(node_ids, registry);
    }

    pub fn reset_with_nodes(&mut self, node_ids: Vec<NodeId>, registry: &mut NodeRegistry) {
        self.input_ids = registry.input_ids_for_step_owned(&node_ids);
        self.focus_targets = Self::focus_targets_for_ids(&node_ids, registry);
        self.focus_index = None;

        if !self.focus_targets.is_empty() {
            self.set_focus_internal(registry, Some(0));
        }
    }

    pub fn focus_index(&self) -> Option<usize> {
        self.focus_index
    }

    pub fn focused_target(&self) -> Option<&FocusTarget> {
        self.focus_index.and_then(|i| self.focus_targets.get(i))
    }

    pub fn focused_id(&self) -> Option<&NodeId> {
        match self.focused_target() {
            Some(FocusTarget::Input(id)) => Some(id),
            _ => None,
        }
    }

    pub fn focused_node_id(&self) -> Option<&NodeId> {
        match self.focused_target() {
            Some(FocusTarget::Input(id)) => Some(id),
            Some(FocusTarget::Component(id)) => Some(id),
            None => None,
        }
    }

    pub fn focused_component_id(&self) -> Option<&NodeId> {
        match self.focused_target() {
            Some(FocusTarget::Component(id)) => Some(id),
            _ => None,
        }
    }

    pub fn focused_input<'a>(&self, registry: &'a NodeRegistry) -> Option<&'a dyn Input> {
        self.focused_id().and_then(|id| registry.get_input(id))
    }

    pub fn focused_input_mut<'a>(
        &self,
        registry: &'a mut NodeRegistry,
    ) -> Option<&'a mut dyn Input> {
        let id = self.focused_id()?.clone();
        registry.get_input_mut(&id)
    }

    pub fn focused_component_mut<'a>(
        &self,
        registry: &'a mut NodeRegistry,
    ) -> Option<&'a mut dyn crate::core::component::Component> {
        let id = self.focused_component_id()?.clone();
        registry.get_component_mut(&id)
    }

    pub fn handle_tab_completion(&mut self, registry: &mut NodeRegistry) -> bool {
        let Some(input) = self.focused_input_mut(registry) else {
            return false;
        };

        if !input.supports_tab_completion() {
            return false;
        }

        input.handle_tab_completion()
    }

    pub fn focused_caps(&self, registry: &NodeRegistry) -> Option<InputCaps> {
        self.focused_input(registry).map(|i| i.capabilities())
    }

    pub fn move_focus(&mut self, registry: &mut NodeRegistry, direction: isize) -> Vec<FormEvent> {
        if self.focus_targets.is_empty() {
            return vec![];
        }

        let mut events = Vec::new();
        if let Some(input) = self.focused_input_mut(registry) {
            if let Err(err) = validation::validate_input(input) {
                input.set_error(Some(InputError::hidden(err)));
            } else {
                input.clear_error();
            }
        }

        let current = self.focus_index.unwrap_or(0);
        let len = self.focus_targets.len() as isize;
        let next = ((current as isize + direction + len) % len) as usize;

        self.set_focus(registry, Some(next), &mut events);
        events
    }

    pub fn set_focus(
        &mut self,
        registry: &mut NodeRegistry,
        new_index: Option<usize>,
        events: &mut Vec<FormEvent>,
    ) {
        let from_target = self.focused_target().cloned();
        let to_target = new_index.and_then(|i| self.focus_targets.get(i)).cloned();

        if from_target == to_target {
            return;
        }

        if let Some(target) = &from_target {
            self.set_target_focus(registry, target, false);
        }

        if let Some(target) = &to_target {
            self.set_target_focus(registry, target, true);
        }

        self.focus_index = new_index;
        let from_id = from_target.and_then(|t| t.node_id().cloned());
        let to_id = to_target.and_then(|t| t.node_id().cloned());
        events.push(FormEvent::FocusChanged {
            from: from_id,
            to: to_id,
        });
    }

    pub fn clear_focus(&mut self, registry: &mut NodeRegistry) {
        if let Some(target) = self.focused_target().cloned() {
            self.set_target_focus(registry, &target, false);
        }
        self.focus_index = None;
    }

    pub fn find_index_by_id(&self, id: &str) -> Option<usize> {
        self.focus_targets
            .iter()
            .position(|target| target.node_id().map(|nid| nid == id).unwrap_or(false))
    }

    pub fn handle_key(&mut self, registry: &mut NodeRegistry, key: KeyEvent) -> Vec<FormEvent> {
        self.update_focused_input(registry, |input| {
            Some(input.handle_key(key.code, key.modifiers))
        })
    }

    pub fn handle_delete_word(
        &mut self,
        registry: &mut NodeRegistry,
        forward: bool,
    ) -> Vec<FormEvent> {
        self.update_focused_input(registry, |input| {
            if forward {
                input.delete_word_forward();
            } else {
                input.delete_word();
            }
            None
        })
    }

    pub fn validate_focused(&self, registry: &mut NodeRegistry) -> Result<(), (NodeId, String)> {
        let Some(id) = self.focused_id().cloned() else {
            return Ok(());
        };

        let Some(input) = registry.get_input_mut(&id) else {
            return Ok(());
        };

        match validation::validate_input(input) {
            Ok(()) => {
                input.clear_error();
                Ok(())
            }
            Err(err) => {
                input.set_error(Some(InputError::inline(&err)));
                Err((id, err))
            }
        }
    }

    pub fn apply_errors(
        &mut self,
        registry: &mut NodeRegistry,
        errors: &[(NodeId, String)],
    ) -> Vec<NodeId> {
        let mut scheduled = Vec::new();

        for id in &self.input_ids {
            let Some(input) = registry.get_input_mut(id) else {
                continue;
            };

            if let Some((_, err)) = errors.iter().find(|(eid, _)| eid == id) {
                input.set_error(Some(InputError::inline(err)));
                scheduled.push(id.clone());
            } else {
                input.clear_error();
            }
        }

        scheduled
    }

    pub fn clear_error(&self, registry: &mut NodeRegistry, id: &str) {
        if let Some(input) = registry.get_input_mut(id) {
            input.clear_error();
        }
    }

    pub fn advance_focus(
        &mut self,
        registry: &mut NodeRegistry,
        events: &mut Vec<FormEvent>,
    ) -> bool {
        let Some(current) = self.focus_index else {
            return false;
        };

        let next = current + 1;
        if next < self.focus_targets.len() {
            self.set_focus(registry, Some(next), events);
            true
        } else {
            false
        }
    }

    pub fn input_ids(&self) -> &[NodeId] {
        &self.input_ids
    }

    fn set_focus_internal(&mut self, registry: &mut NodeRegistry, new_index: Option<usize>) {
        if let Some(target) = self.focused_target().cloned() {
            self.set_target_focus(registry, &target, false);
        }

        if let Some(idx) = new_index {
            if let Some(target) = self.focus_targets.get(idx) {
                self.set_target_focus(registry, target, true);
            }
        }

        self.focus_index = new_index;
    }

    fn update_focused_input<F>(&mut self, registry: &mut NodeRegistry, update: F) -> Vec<FormEvent>
    where
        F: FnOnce(&mut dyn Input) -> Option<KeyResult>,
    {
        let Some(id) = self.focused_id().cloned() else {
            return vec![];
        };

        let Some(input) = registry.get_input_mut(&id) else {
            return vec![];
        };

        let before = input.value();
        let result = update(input);
        let after = input.value();

        let mut events = Vec::new();

        if before != after {
            events.push(FormEvent::InputChanged {
                id: id.clone(),
                value: after,
            });
            events.push(FormEvent::ErrorCancelled { id: id.clone() });
            input.clear_error();
        }

        if let Err(err) = validation::validate_input(input) {
            input.set_error(Some(InputError::hidden(err)));
        }

        if matches!(result, Some(KeyResult::Submit)) {
            events.push(FormEvent::SubmitRequested);
        }

        events
    }

    fn focus_targets_for_ids(node_ids: &[NodeId], registry: &NodeRegistry) -> Vec<FocusTarget> {
        let mut targets = Vec::new();
        for id in node_ids {
            let Some(node) = registry.get(id) else {
                continue;
            };

            if node.is_input() {
                targets.push(FocusTarget::Input(id.clone()));
            } else if node.is_component() {
                targets.push(FocusTarget::Component(id.clone()));
            }
        }

        targets
    }

    fn set_target_focus(&self, registry: &mut NodeRegistry, target: &FocusTarget, focused: bool) {
        match target {
            FocusTarget::Input(id) => {
                if let Some(input) = registry.get_input_mut(id) {
                    input.set_focused(focused);
                }
            }
            FocusTarget::Component(id) => {
                if let Some(component) = registry.get_component_mut(id) {
                    component.set_focused(focused);
                }
            }
        }
    }
}

impl FocusTarget {
    pub fn node_id(&self) -> Option<&NodeId> {
        match self {
            FocusTarget::Input(id) => Some(id),
            FocusTarget::Component(id) => Some(id),
        }
    }
}
