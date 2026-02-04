use crate::core::form_event::FormEvent;
use crate::core::node::NodeId;
use crate::core::node_registry::NodeRegistry;
use crate::core::step::Step;
use crate::core::validation;
use crate::inputs::{Input, InputCaps, InputError, KeyResult};
use crate::terminal::KeyEvent;

pub struct FormEngine {
    input_ids: Vec<NodeId>,
    focus_index: Option<usize>,
}

impl FormEngine {
    pub fn new(step: &Step, registry: &mut NodeRegistry) -> Self {
        let input_ids = registry.input_ids_for_step_owned(&step.node_ids);
        Self::from_input_ids(input_ids, registry)
    }

    pub fn from_input_ids(input_ids: Vec<NodeId>, registry: &mut NodeRegistry) -> Self {
        let mut engine = Self {
            input_ids,
            focus_index: None,
        };

        if !engine.input_ids.is_empty() {
            engine.set_focus_internal(registry, Some(0));
        }

        engine
    }

    pub fn reset(&mut self, step: &Step, registry: &mut NodeRegistry) {
        let input_ids = registry.input_ids_for_step_owned(&step.node_ids);
        self.reset_with_ids(input_ids, registry);
    }

    pub fn reset_with_ids(&mut self, input_ids: Vec<NodeId>, registry: &mut NodeRegistry) {
        self.input_ids = input_ids;
        self.focus_index = None;

        if !self.input_ids.is_empty() {
            self.set_focus_internal(registry, Some(0));
        }
    }


    pub fn focus_index(&self) -> Option<usize> {
        self.focus_index
    }

    pub fn focused_id(&self) -> Option<&NodeId> {
        self.focus_index.and_then(|i| self.input_ids.get(i))
    }

    pub fn focused_input<'a>(&self, registry: &'a NodeRegistry) -> Option<&'a dyn Input> {
        self.focused_id().and_then(|id| registry.get_input(id))
    }

    pub fn focused_input_mut<'a>(&self, registry: &'a mut NodeRegistry) -> Option<&'a mut dyn Input> {
        let id = self.focused_id()?.clone();
        registry.get_input_mut(&id)
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
        if self.input_ids.is_empty() {
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
        let len = self.input_ids.len() as isize;
        let next = ((current as isize + direction + len) % len) as usize;

        self.set_focus(registry, Some(next), &mut events);
        events
    }

    pub fn set_focus(&mut self, registry: &mut NodeRegistry, new_index: Option<usize>, events: &mut Vec<FormEvent>) {
        let from_id = self.focused_id().cloned();
        let to_id = new_index.and_then(|i| self.input_ids.get(i)).cloned();

        if from_id == to_id {
            return;
        }

        if let Some(id) = &from_id {
            if let Some(input) = registry.get_input_mut(id) {
                input.set_focused(false);
            }
        }

        if let Some(id) = &to_id {
            if let Some(input) = registry.get_input_mut(id) {
                input.set_focused(true);
            }
        }

        self.focus_index = new_index;
        events.push(FormEvent::FocusChanged { from: from_id, to: to_id });
    }

    pub fn clear_focus(&mut self, registry: &mut NodeRegistry) {
        if let Some(id) = self.focused_id() {
            if let Some(input) = registry.get_input_mut(id) {
                input.set_focused(false);
            }
        }
        self.focus_index = None;
    }

    pub fn find_index_by_id(&self, id: &str) -> Option<usize> {
        self.input_ids.iter().position(|i| i == id)
    }


    pub fn handle_key(&mut self, registry: &mut NodeRegistry, key: KeyEvent) -> Vec<FormEvent> {
        self.update_focused_input(registry, |input| {
            Some(input.handle_key(key.code, key.modifiers))
        })
    }

    pub fn handle_delete_word(&mut self, registry: &mut NodeRegistry, forward: bool) -> Vec<FormEvent> {
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

    pub fn apply_errors(&mut self, registry: &mut NodeRegistry, errors: &[(NodeId, String)]) -> Vec<NodeId> {
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


    pub fn advance_focus(&mut self, registry: &mut NodeRegistry, events: &mut Vec<FormEvent>) -> bool {
        let Some(current) = self.focus_index else {
            return false;
        };

        let next = current + 1;
        if next < self.input_ids.len() {
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
        if let Some(id) = self.focused_id() {
            if let Some(input) = registry.get_input_mut(id) {
                input.set_focused(false);
            }
        }

        if let Some(idx) = new_index {
            if let Some(id) = self.input_ids.get(idx) {
                if let Some(input) = registry.get_input_mut(id) {
                    input.set_focused(true);
                }
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
            events.push(FormEvent::InputChanged { id: id.clone(), value: after });
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
}
