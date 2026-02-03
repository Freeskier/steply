use crate::core::validation;
use crate::event_queue::AppEvent;
use crate::inputs::{Input, InputCaps, KeyResult};
use crate::node::Node;
use crate::step::Step;
use crate::terminal::KeyEvent;
use crate::view_state::{ErrorDisplay, ViewState};

#[derive(Default)]
pub struct FormResult {
    pub events: Vec<AppEvent>,
    pub cancel_clear_error_for: Option<String>,
    pub submit_requested: bool,
}

pub struct FormEngine {
    input_node_indices: Vec<usize>,
    focused_index: Option<usize>,
}

impl FormEngine {
    pub fn new(step: &mut Step) -> Self {
        let input_node_indices: Vec<usize> = step
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                if matches!(node, Node::Input(_)) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        let mut engine = Self {
            input_node_indices,
            focused_index: None,
        };

        if !engine.input_node_indices.is_empty() {
            engine.set_focus_without_events(step, Some(0));
        }

        engine
    }

    pub fn reset(&mut self, step: &mut Step) {
        self.input_node_indices = step
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                if matches!(node, Node::Input(_)) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        self.focused_index = None;

        if !self.input_node_indices.is_empty() {
            self.set_focus_without_events(step, Some(0));
        }
    }

    pub fn clear_focus(&mut self, step: &mut Step) {
        if let Some(old_index) = self.focused_index {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[old_index])
            {
                input.set_focused(false);
            }
        }
        self.focused_index = None;
    }

    pub fn focused_input_id(&self, step: &Step) -> Option<String> {
        self.focused_index
            .and_then(|index| self.input_id_at(step, index))
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub fn focused_input_caps(&self, step: &Step) -> Option<InputCaps> {
        self.focused_index
            .and_then(|index| step.nodes.get(self.input_node_indices[index]))
            .and_then(|node| node.as_input())
            .map(|input| input.capabilities())
    }

    pub fn focused_input<'a>(&self, step: &'a Step) -> Option<&'a dyn Input> {
        self.focused_index
            .and_then(|index| step.nodes.get(self.input_node_indices[index]))
            .and_then(|node| node.as_input())
    }

    pub fn focused_input_mut<'a>(
        &mut self,
        step: &'a mut Step,
        pos: usize,
    ) -> Option<&'a mut dyn Input> {
        step.nodes
            .get_mut(self.input_node_indices[pos])
            .and_then(|node| node.as_input_mut())
    }

    pub fn handle_input_key(
        &mut self,
        step: &mut Step,
        key_event: KeyEvent,
        view_state: &mut ViewState,
    ) -> FormResult {
        let mut result = FormResult::default();

        let Some(current_index) = self.focused_index else {
            return result;
        };

        let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[current_index])
        else {
            return result;
        };

        let before = input.value();
        let key_result = input.handle_key(key_event.code, key_event.modifiers);
        let after = input.value();
        let id = input.id().clone();
        let validation_result = validation::validate_input(input.as_ref());

        if before != after {
            result.events.push(AppEvent::InputChanged {
                id: id.clone(),
                value: after,
            });
            result.cancel_clear_error_for = Some(id.clone());
        }

        if matches!(key_result, KeyResult::Submit) {
            result.submit_requested = true;
        }

        self.clear_error_message(view_state, &id);
        self.apply_validation_result(step, view_state, &id, validation_result);

        result
    }

    pub fn handle_delete_word(
        &mut self,
        step: &mut Step,
        forward: bool,
        view_state: &mut ViewState,
    ) -> FormResult {
        let mut result = FormResult::default();

        let Some(current_index) = self.focused_index else {
            return result;
        };

        let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[current_index])
        else {
            return result;
        };

        let before = input.value();
        if forward {
            input.delete_word_forward();
        } else {
            input.delete_word();
        }
        let after = input.value();
        let id = input.id().clone();
        let validation_result = validation::validate_input(input.as_ref());

        if before != after {
            result.events.push(AppEvent::InputChanged {
                id: id.clone(),
                value: after,
            });
            result.cancel_clear_error_for = Some(id.clone());
        }

        self.clear_error_message(view_state, &id);
        self.apply_validation_result(step, view_state, &id, validation_result);

        result
    }

    pub fn move_focus(
        &mut self,
        step: &mut Step,
        direction: isize,
        view_state: &mut ViewState,
    ) -> FormResult {
        let mut result = FormResult::default();

        if self.input_node_indices.is_empty() {
            return result;
        }

        if let Some(current_index) = self.focused_index {
            if let Some(Node::Input(input)) =
                step.nodes.get_mut(self.input_node_indices[current_index])
            {
                let id = input.id().clone();
                let validation_result = validation::validate_input(input.as_ref());
                self.apply_validation_result(step, view_state, &id, validation_result);
            }
        }

        let current_index = self.focused_index.unwrap_or(0);
        let len = self.input_node_indices.len() as isize;
        let next_index = (current_index as isize + direction + len) % len;
        self.update_focus(step, Some(next_index as usize), &mut result.events);

        result
    }

    pub fn handle_clear_error_message(
        &mut self,
        step: &mut Step,
        id: &str,
        view_state: &mut ViewState,
    ) {
        if let Some(pos) = self.find_input_pos_by_id(step, id) {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[pos]) {
                view_state.clear_error_display(input.id());
            }
        }
    }

    pub fn apply_validation_errors(
        &mut self,
        step: &mut Step,
        errors: &[(String, String)],
        view_state: &mut ViewState,
    ) -> Vec<String> {
        let mut scheduled = Vec::new();
        for idx in &self.input_node_indices {
            if let Some(Node::Input(input)) = step.nodes.get_mut(*idx) {
                if let Some((_, error)) = errors.iter().find(|(id, _)| id == input.id()) {
                    let id = input.id().clone();
                    input.set_error(Some(error.clone()));
                    view_state.set_error_display(id.clone(), ErrorDisplay::InlineMessage);
                    scheduled.push(id);
                } else {
                    input.set_error(None);
                    view_state.clear_error_display(input.id());
                }
            }
        }
        scheduled
    }

    pub fn advance_focus_after_submit(
        &mut self,
        step: &mut Step,
        events: &mut Vec<AppEvent>,
    ) -> bool {
        let Some(current_index) = self.focused_index else {
            return false;
        };
        let next_index = current_index + 1;
        if next_index < self.input_node_indices.len() {
            self.update_focus(step, Some(next_index), events);
            true
        } else {
            false
        }
    }

    pub fn find_input_pos_by_id(&self, step: &Step, id: &str) -> Option<usize> {
        self.input_node_indices.iter().position(|idx| {
            step.nodes
                .get(*idx)
                .and_then(|node| node.as_input())
                .is_some_and(|input| input.id() == id)
        })
    }

    pub fn update_focus(
        &mut self,
        step: &mut Step,
        new_pos: Option<usize>,
        events: &mut Vec<AppEvent>,
    ) {
        let from_id = self
            .focused_index
            .and_then(|index| self.input_id_at(step, index));
        let to_id = new_pos.and_then(|index| self.input_id_at(step, index));

        if let Some(old_index) = self.focused_index {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[old_index])
            {
                input.set_focused(false);
            }
        }

        if let Some(index) = new_pos {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[index]) {
                input.set_focused(true);
            }
        }

        self.focused_index = new_pos;
        if from_id != to_id {
            events.push(AppEvent::FocusChanged {
                from: from_id,
                to: to_id,
            });
        }
    }

    fn apply_validation_result(
        &mut self,
        step: &mut Step,
        view_state: &mut ViewState,
        id: &str,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => {
                if let Some(pos) = self.find_input_pos_by_id(step, id) {
                    if let Some(Node::Input(input_mut)) =
                        step.nodes.get_mut(self.input_node_indices[pos])
                    {
                        input_mut.set_error(None);
                    }
                }
                view_state.clear_error_display(id);
            }
            Err(err) => {
                if let Some(pos) = self.find_input_pos_by_id(step, id) {
                    if let Some(Node::Input(input_mut)) =
                        step.nodes.get_mut(self.input_node_indices[pos])
                    {
                        input_mut.set_error(Some(err.clone()));
                    }
                }
                view_state.clear_error_display(id);
            }
        }
    }

    fn clear_error_message(&mut self, view_state: &mut ViewState, id: &str) {
        view_state.clear_error_display(id);
    }

    fn set_focus_without_events(&mut self, step: &mut Step, new_pos: Option<usize>) {
        if let Some(old_index) = self.focused_index {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[old_index])
            {
                input.set_focused(false);
            }
        }

        if let Some(index) = new_pos {
            if let Some(Node::Input(input)) = step.nodes.get_mut(self.input_node_indices[index]) {
                input.set_focused(true);
            }
        }

        self.focused_index = new_pos;
    }

    fn input_id_at(&self, step: &Step, index: usize) -> Option<String> {
        self.input_node_indices
            .get(index)
            .and_then(|idx| step.nodes.get(*idx))
            .and_then(|node| node.as_input())
            .map(|input| input.id().clone())
    }
}
