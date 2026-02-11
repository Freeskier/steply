use crate::core::event::Command;
use crate::core::event_queue::AppEvent;
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
        command: Command,
        error_timeout: Duration,
        mut active_nodes: Option<&mut [Node]>,
    ) -> Vec<Effect> {
        match command {
            Command::Exit => {
                state.should_exit = true;
                vec![]
            }
            Command::Cancel => {
                state.flow.cancel_current();
                state.should_exit = true;
                vec![]
            }
            Command::NextInput => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.move_focus(nodes, 1);
                Self::focus_changes_to_effects(events)
            }
            Command::PrevInput => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.move_focus(nodes, -1);
                Self::focus_changes_to_effects(events)
            }
            Command::Submit => {
                let mut effects = Vec::new();
                Self::append_submit_effects(state, error_timeout, &mut active_nodes, &mut effects);
                effects
            }
            Command::DeleteWord => {
                let output = {
                    let nodes = match active_nodes.as_deref_mut() {
                        Some(nodes) => nodes,
                        None => state.flow.current_step_mut().nodes.as_mut_slice(),
                    };
                    state.engine.handle_delete_word(nodes, false)
                };
                Self::handle_engine_output_with_submit(
                    state,
                    error_timeout,
                    &mut active_nodes,
                    output,
                )
            }
            Command::DeleteWordForward => {
                let output = {
                    let nodes = match active_nodes.as_deref_mut() {
                        Some(nodes) => nodes,
                        None => state.flow.current_step_mut().nodes.as_mut_slice(),
                    };
                    state.engine.handle_delete_word(nodes, true)
                };
                Self::handle_engine_output_with_submit(
                    state,
                    error_timeout,
                    &mut active_nodes,
                    output,
                )
            }
            Command::InputKey(key_event) => {
                let output = {
                    let nodes = match active_nodes.as_deref_mut() {
                        Some(nodes) => nodes,
                        None => state.flow.current_step_mut().nodes.as_mut_slice(),
                    };
                    state.engine.handle_key(nodes, key_event)
                };
                Self::handle_engine_output_with_submit(
                    state,
                    error_timeout,
                    &mut active_nodes,
                    output,
                )
            }
            Command::TabKey(key_event) => {
                let tab_completed = {
                    let nodes = match active_nodes.as_deref_mut() {
                        Some(nodes) => nodes,
                        None => state.flow.current_step_mut().nodes.as_mut_slice(),
                    };
                    state.engine.handle_tab_completion(nodes)
                };
                if tab_completed {
                    return vec![];
                }

                let output = {
                    let nodes = match active_nodes.as_deref_mut() {
                        Some(nodes) => nodes,
                        None => state.flow.current_step_mut().nodes.as_mut_slice(),
                    };
                    state.engine.handle_key(nodes, key_event)
                };
                if output.handled {
                    return Self::handle_engine_output_with_submit(
                        state,
                        error_timeout,
                        &mut active_nodes,
                        output,
                    );
                }

                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                let events = state.engine.move_focus(nodes, 1);
                Self::focus_changes_to_effects(events)
            }
            Command::ClearErrorMessage(id) => {
                let nodes = match active_nodes.as_deref_mut() {
                    Some(nodes) => nodes,
                    None => state.flow.current_step_mut().nodes.as_mut_slice(),
                };
                state.engine.clear_error(nodes, &id);
                vec![]
            }
        }
    }

    fn focus_changes_to_effects(changes: Vec<(Option<NodeId>, Option<NodeId>)>) -> Vec<Effect> {
        let mut effects = Vec::new();

        for (from, to) in changes {
            effects.push(Effect::Emit(AppEvent::FocusChanged { from, to }));
        }

        effects
    }

    fn reduce_engine_output(output: crate::core::form_engine::EngineOutput) -> Vec<Effect> {
        let mut effects = Vec::new();
        for (id, value) in output.input_changes {
            effects.push(Effect::Emit(AppEvent::InputChanged {
                id: id.clone(),
                value,
            }));
            effects.push(Effect::CancelClearError(id));
        }

        for produced in output.produced {
            effects.push(Effect::ComponentProduced {
                id: produced.id,
                value: produced.value,
            });
        }

        effects
    }

    fn handle_engine_output_with_submit(
        state: &mut AppState,
        error_timeout: Duration,
        active_nodes: &mut Option<&mut [Node]>,
        output: crate::core::form_engine::EngineOutput,
    ) -> Vec<Effect> {
        let submit_requested = output.submit_requested;
        let mut effects = Self::reduce_engine_output(output);
        if submit_requested {
            Self::append_submit_effects(state, error_timeout, active_nodes, &mut effects);
        }
        effects
    }

    fn append_submit_effects(
        state: &mut AppState,
        error_timeout: Duration,
        active_nodes: &mut Option<&mut [Node]>,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(nodes) = active_nodes.as_deref_mut() {
            effects.extend(Self::handle_submit_in_active_nodes(
                state,
                error_timeout,
                nodes,
            ));
        } else {
            effects.extend(Self::handle_submit(state, error_timeout));
        }
    }

    fn handle_submit_in_active_nodes(
        state: &mut AppState,
        error_timeout: Duration,
        nodes: &mut [Node],
    ) -> Vec<Effect> {
        let mut effects = Vec::new();

        if let Err((id, _err)) = state.engine.validate_focused(nodes) {
            effects.push(Effect::CancelClearError(id.clone()));
            effects.push(Effect::EmitAfter(
                AppEvent::ClearErrorMessage(id),
                error_timeout,
            ));
            return effects;
        }

        let mut focus_events = Vec::new();
        if state.engine.advance_focus(nodes, &mut focus_events) {
            effects.extend(Self::focus_changes_to_effects(focus_events));
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
                AppEvent::ClearErrorMessage(id),
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
            effects.extend(Self::focus_changes_to_effects(focus_events));
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
                AppEvent::ClearErrorMessage(id),
                error_timeout,
            ));
        }

        if let Some((first_id, _)) = errors.first() {
            if let Some(idx) = state.engine.find_index_by_id(first_id) {
                let mut focus_events = Vec::new();
                let nodes = state.flow.current_step_mut().nodes.as_mut_slice();
                state.engine.set_focus(nodes, Some(idx), &mut focus_events);
                effects.extend(Self::focus_changes_to_effects(focus_events));
            }
        }
    }
}
