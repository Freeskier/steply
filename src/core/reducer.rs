use crate::core::event::Action;
use crate::core::event_queue::AppEvent;
use crate::core::form_event::FormEvent;
use crate::core::node::NodeId;
use crate::core::state::AppState;
use crate::core::validation;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Effect {
    Emit(AppEvent),
    EmitAfter(AppEvent, Duration),
    CancelClearError(String),
}

pub struct Reducer;

impl Reducer {
    pub fn reduce(state: &mut AppState, action: Action, error_timeout: Duration) -> Vec<Effect> {
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
                let registry = state.flow.registry_mut();
                let events = state.engine.move_focus(registry, 1);
                Self::form_events_to_effects(events)
            }
            Action::PrevInput => {
                let registry = state.flow.registry_mut();
                let events = state.engine.move_focus(registry, -1);
                Self::form_events_to_effects(events)
            }
            Action::Submit => Self::handle_submit(state, error_timeout),
            Action::DeleteWord => {
                let registry = state.flow.registry_mut();
                let events = state.engine.handle_delete_word(registry, false);
                Self::form_events_to_effects(events)
            }
            Action::DeleteWordForward => {
                let registry = state.flow.registry_mut();
                let events = state.engine.handle_delete_word(registry, true);
                Self::form_events_to_effects(events)
            }
            Action::InputKey(key_event) => {
                let registry = state.flow.registry_mut();
                let events = state.engine.handle_key(registry, key_event);

                let has_submit = events.iter().any(|e| matches!(e, FormEvent::SubmitRequested));
                let mut effects = Self::form_events_to_effects(events);

                if has_submit {
                    effects.extend(Self::handle_submit(state, error_timeout));
                }

                effects
            }
            Action::ClearErrorMessage(id) => {
                let registry = state.flow.registry_mut();
                state.engine.clear_error(registry, &id);
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
                FormEvent::SubmitRequested => {
                }
            }
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
        let registry = state.flow.registry_mut();
        if let Err((id, _err)) = state.engine.validate_focused(registry) {
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
        let registry = state.flow.registry_mut();
        let mut focus_events = Vec::new();
        if state.engine.advance_focus(registry, &mut focus_events) {
            effects.extend(Self::form_events_to_effects(focus_events));
            return true;
        }
        false
    }

    fn validate_current_step(state: &AppState) -> Vec<(NodeId, String)> {
        let step = state.flow.current_step();
        let registry = state.flow.registry();
        validation::validate_all_inputs(step, registry)
    }

    fn handle_successful_submit(state: &mut AppState, mut effects: Vec<Effect>) -> Vec<Effect> {
        if state.flow.has_next() {
            let registry = state.flow.registry_mut();
            state.engine.clear_focus(registry);
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
        let registry = state.flow.registry_mut();
        let scheduled_ids = state.engine.apply_errors(registry, errors);

        for id in scheduled_ids {
            effects.push(Effect::EmitAfter(
                AppEvent::Action(Action::ClearErrorMessage(id)),
                error_timeout,
            ));
        }

        if let Some((first_id, _)) = errors.first() {
            if let Some(idx) = state.engine.find_index_by_id(first_id) {
                let registry = state.flow.registry_mut();
                let mut focus_events = Vec::new();
                state.engine.set_focus(registry, Some(idx), &mut focus_events);
                effects.extend(Self::form_events_to_effects(focus_events));
            }
        }
    }
}
