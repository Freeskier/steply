use crate::core::event::Action;
use crate::core::event_queue::AppEvent;
use crate::core::state::AppState;
use crate::core::validation;
use crate::view_state::ErrorDisplay;
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
            Action::NextInput => {
                let result = state.engine.move_focus(1, &mut state.view);
                let (effects, _) = Self::effects_from_form_result(result);
                effects
            }
            Action::PrevInput => {
                let result = state.engine.move_focus(-1, &mut state.view);
                let (effects, _) = Self::effects_from_form_result(result);
                effects
            }
            Action::Submit => Self::handle_submit(state, error_timeout),
            Action::DeleteWord => {
                let result = state.engine.handle_delete_word(false, &mut state.view);
                let (effects, _) = Self::effects_from_form_result(result);
                effects
            }
            Action::DeleteWordForward => {
                let result = state.engine.handle_delete_word(true, &mut state.view);
                let (effects, _) = Self::effects_from_form_result(result);
                effects
            }
            Action::InputKey(key_event) => {
                let result = state.engine.handle_input_key(key_event, &mut state.view);
                let (mut effects, submit_requested) = Self::effects_from_form_result(result);
                if submit_requested {
                    effects.extend(Self::handle_submit(state, error_timeout));
                }
                effects
            }
            Action::ClearErrorMessage(id) => {
                state
                    .engine
                    .handle_clear_error_message(&id, &mut state.view);
                vec![]
            }
        }
    }

    fn effects_from_form_result(
        result: crate::core::form_engine::FormResult,
    ) -> (Vec<Effect>, bool) {
        let mut effects: Vec<Effect> = result.events.into_iter().map(Effect::Emit).collect();

        if let Some(id) = result.cancel_clear_error_for {
            effects.push(Effect::CancelClearError(id));
        }

        (effects, result.submit_requested)
    }

    fn handle_submit(state: &mut AppState, error_timeout: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();

        if let Some(current_index) = state.engine.focused_index() {
            if let Some(input) = state.engine.focused_input_mut(current_index) {
                if let Err(err) = validation::validate_input(input) {
                    let id = input.id().clone();
                    input.set_error(Some(err.clone()));
                    state
                        .view
                        .set_error_display(id.clone(), ErrorDisplay::InlineMessage);

                    effects.push(Effect::CancelClearError(id.clone()));
                    effects.push(Effect::EmitAfter(
                        AppEvent::Action(Action::ClearErrorMessage(id.clone())),
                        error_timeout,
                    ));
                    return effects;
                }

                input.set_error(None);
                state.view.clear_error_display(input.id());
                effects.push(Effect::CancelClearError(input.id().clone()));

                let mut focus_events = Vec::new();
                if state.engine.advance_focus_after_submit(&mut focus_events) {
                    effects.extend(focus_events.into_iter().map(Effect::Emit));
                    return effects;
                }

                let errors = validation::validate_all(&state.engine.step);
                if errors.is_empty() {
                    effects.push(Effect::Emit(AppEvent::Submitted));
                    state.should_exit = true;
                    return effects;
                }

                let scheduled_ids = state
                    .engine
                    .apply_validation_errors(&errors, &mut state.view);
                for id in scheduled_ids {
                    effects.push(Effect::EmitAfter(
                        AppEvent::Action(Action::ClearErrorMessage(id.clone())),
                        error_timeout,
                    ));
                }

                if let Some(first_id) = errors.first().map(|(id, _)| id.clone()) {
                    if let Some(pos) = state.engine.find_input_pos_by_id(&first_id) {
                        let mut focus_events = Vec::new();
                        state.engine.update_focus(Some(pos), &mut focus_events);
                        effects.extend(focus_events.into_iter().map(Effect::Emit));
                    }
                }
            }
        }

        effects
    }
}
