use crate::core::event::Action;
use crate::core::event_queue::AppEvent;
use crate::core::form_event::FormEvent;
use crate::core::node::Node;
use crate::core::node::NodeId;
use crate::core::state::AppState;
use crate::core::validation;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Effect {
    Emit(AppEvent),
    EmitAfter(AppEvent, Duration),
    CancelClearError(String),
    ComponentProduced {
        id: NodeId,
        value: crate::core::value::Value,
    },
}

pub struct Reducer;

impl Reducer {
    pub fn reduce(
        state: &mut AppState,
        action: Action,
        error_timeout: Duration,
        mut active_nodes: Option<&mut [Node]>,
    ) -> Vec<Effect> {
        match action {
            Action::Exit => {
                state.should_exit = true;
                vec![]
            }
            Action::Cancel => {
                state.flow.cancel_current();
                state.should_exit = true;
                vec![]
            }
            Action::NextInput => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.move_focus(nodes, 1);
                Self::form_events_to_effects(events)
            }
            Action::PrevInput => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.move_focus(nodes, -1);
                Self::form_events_to_effects(events)
            }
            Action::Submit => Self::handle_submit(state, error_timeout),
            Action::DeleteWord => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.handle_delete_word(nodes, false);
                Self::form_events_to_effects(events)
            }
            Action::DeleteWordForward => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.handle_delete_word(nodes, true);
                Self::form_events_to_effects(events)
            }
            Action::InputKey(key_event) => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let output = state.engine.handle_key(nodes, key_event);
                Self::reduce_engine_output(state, output, error_timeout)
            }
            Action::TabKey(key_event) => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };

                if state.engine.handle_tab_completion(nodes) {
                    return vec![];
                }

                let output = state.engine.handle_key(nodes, key_event);
                if output.handled {
                    return Self::reduce_engine_output(state, output, error_timeout);
                }

                let events = state.engine.move_focus(nodes, 1);
                Self::form_events_to_effects(events)
            }
            Action::ClearErrorMessage(id) => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                state.engine.clear_error(nodes, &id);
                vec![]
            }
        }
    }

    fn form_events_to_effects(events: Vec<FormEvent>) -> Vec<Effect> {
        let mut effects = Vec::new();

        for event in events {
            match event {
                FormEvent::InputChanged { id, value } => {
                    effects.push(Effect::Emit(AppEvent::InputChanged { id, value }));
                }
                FormEvent::FocusChanged { from, to } => {
                    effects.push(Effect::Emit(AppEvent::FocusChanged { from, to }));
                }
                FormEvent::ErrorCancelled { id } => {
                    effects.push(Effect::CancelClearError(id));
                }
                FormEvent::ErrorScheduled { id } => {
                    effects.push(Effect::CancelClearError(id));
                }
                FormEvent::SubmitRequested => {}
            }
        }

        effects
    }

    fn reduce_engine_output(
        state: &mut AppState,
        output: crate::core::form_engine::EngineOutput,
        error_timeout: Duration,
    ) -> Vec<Effect> {
        let has_submit = output
            .events
            .iter()
            .any(|e| matches!(e, FormEvent::SubmitRequested));

        let mut effects = Self::form_events_to_effects(output.events);
        for produced in output.produced {
            effects.push(Effect::ComponentProduced {
                id: produced.id,
                value: produced.value,
            });
        }

        if has_submit {
            effects.extend(Self::handle_submit(state, error_timeout));
        }

        effects
    }

    fn handle_submit(state: &mut AppState, error_timeout: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();

        if Self::validate_focused(state, error_timeout, &mut effects) {
            return effects;
        }

        if Self::advance_focus(state, &mut effects) {
            return effects;
        }

        let errors = Self::validate_current_step(state);
        if errors.is_empty() {
            return Self::handle_successful_submit(state, effects);
        }

        Self::apply_errors_and_focus(state, &errors, error_timeout, &mut effects);
        effects
    }

    fn validate_focused(
        state: &mut AppState,
        error_timeout: Duration,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
        if let Err((id, _err)) = state.engine.validate_focused(nodes) {
            effects.push(Effect::CancelClearError(id.clone()));
            effects.push(Effect::EmitAfter(
                AppEvent::Action(Action::ClearErrorMessage(id)),
                error_timeout,
            ));
            return true;
        }
        false
    }

    fn advance_focus(state: &mut AppState, effects: &mut Vec<Effect>) -> bool {
        let mut focus_events = Vec::new();
        let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
        if state.engine.advance_focus(nodes, &mut focus_events) {
            effects.extend(Self::form_events_to_effects(focus_events));
            return true;
        }
        false
    }

    fn validate_current_step(state: &AppState) -> Vec<(NodeId, String)> {
        let step = state.flow.current_step();
        validation::validate_all_inputs(step)
    }

    fn handle_successful_submit(state: &mut AppState, mut effects: Vec<Effect>) -> Vec<Effect> {
        if state.flow.has_next() {
            let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
            state.engine.clear_focus(nodes);
            state.flow.advance();
            state.reset_engine_for_current_step();
            return effects;
        }

        effects.push(Effect::Emit(AppEvent::Submitted));
        state.should_exit = true;
        effects
    }

    fn apply_errors_and_focus(
        state: &mut AppState,
        errors: &[(NodeId, String)],
        error_timeout: Duration,
        effects: &mut Vec<Effect>,
    ) {
        let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
        let scheduled_ids = state.engine.apply_errors(nodes, errors);

        for id in scheduled_ids {
            effects.push(Effect::EmitAfter(
                AppEvent::Action(Action::ClearErrorMessage(id)),
                error_timeout,
            ));
        }

        if let Some((first_id, _)) = errors.first() {
            if let Some(idx) = state.engine.find_index_by_id(first_id) {
                let mut focus_events = Vec::new();
                let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
                state.engine.set_focus(nodes, Some(idx), &mut focus_events);
                effects.extend(Self::form_events_to_effects(focus_events));
            }
        }
    }
}
