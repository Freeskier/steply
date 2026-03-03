use crate::runtime::effect::Effect;
use crate::runtime::event::SystemEvent;
use crate::runtime::intent::Intent;
use crate::state::app::{AppState, ExitConfirmChoice};
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::traits::InteractionResult;

pub struct Reducer;

impl Reducer {
    pub fn reduce(state: &mut AppState, intent: Intent) -> Vec<Effect> {
        let mut effects = if state.exit_confirm_active() {
            reduce_with_exit_confirm(state, intent)
        } else {
            match intent {
                Intent::Exit => {
                    state.begin_exit_confirm();
                    vec![Effect::RequestRender]
                }
                Intent::Back => {
                    state.handle_step_back();
                    vec![Effect::RequestRender]
                }
                Intent::Cancel => {
                    if state.back_confirm().is_some() {
                        state.cancel_back_confirm();
                    } else if state.cancel_completion_for_focused() {
                        state.suppress_completion_tab_for_focused();
                    } else if state
                        .dispatch_key_to_focused(KeyEvent {
                            code: KeyCode::Esc,
                            modifiers: KeyModifiers::NONE,
                        })
                        .handled
                    {
                    } else if state.has_active_overlay() {
                        state.close_overlay();
                    }
                    vec![Effect::RequestRender]
                }
                Intent::Submit => {
                    if let Some(result) = state.submit_focused() {
                        let effects = collect_effects(result);
                        if !effects.is_empty() {
                            effects
                        } else {
                            fallback_submit(state)
                        }
                    } else {
                        fallback_submit(state)
                    }
                }
                Intent::ToggleCompletion => {
                    state.toggle_completion_for_focused();
                    vec![Effect::RequestRender]
                }
                Intent::CompleteNext => collect_effects(state.handle_tab_forward()),
                Intent::CompletePrev => collect_effects(state.handle_tab_backward()),
                Intent::NextFocus => {
                    state.focus_next();
                    vec![Effect::RequestRender]
                }
                Intent::PrevFocus => {
                    state.focus_prev();
                    vec![Effect::RequestRender]
                }
                Intent::InputKey(key) => {
                    let result = state.dispatch_key_to_focused(key);
                    if result.handled {
                        collect_effects(result)
                    } else if is_plain_enter(key) {
                        fallback_submit(state)
                    } else {
                        vec![]
                    }
                }
                Intent::TextAction(action) => {
                    collect_effects(state.dispatch_text_action_to_focused(action))
                }
                Intent::OpenOverlay(overlay_id) => {
                    vec![Effect::System(SystemEvent::OpenOverlay { overlay_id })]
                }
                Intent::OpenOverlayAtIndex(index) => {
                    if state.open_overlay_by_index(index) {
                        vec![Effect::RequestRender]
                    } else {
                        vec![]
                    }
                }
                Intent::OpenOverlayShortcut => {
                    if state.open_default_overlay() {
                        vec![Effect::RequestRender]
                    } else {
                        vec![]
                    }
                }
                Intent::CloseOverlay => vec![Effect::System(SystemEvent::CloseOverlay)],
                Intent::ToggleHints => {
                    state.toggle_hints_visibility();
                    vec![Effect::RequestRender]
                }
                Intent::Tick => collect_effects(state.tick_all_nodes()),
                Intent::Noop => vec![],
                Intent::ScrollUp
                | Intent::ScrollDown
                | Intent::ScrollPageUp
                | Intent::ScrollPageDown
                | Intent::CopySelection
                | Intent::Pointer(_) => vec![],
                Intent::PointerOn { target, event } => {
                    collect_effects(state.dispatch_pointer_to_node(target.as_str(), event))
                }
            }
        };

        effects.extend(
            state
                .take_pending_scheduler_commands()
                .into_iter()
                .map(Effect::Schedule),
        );

        effects
    }
}

fn reduce_with_exit_confirm(state: &mut AppState, intent: Intent) -> Vec<Effect> {
    match intent {
        Intent::Exit => {
            state.request_exit();
            vec![Effect::RequestRender]
        }
        Intent::Cancel => {
            state.cancel_exit_confirm();
            vec![Effect::RequestRender]
        }
        Intent::InputKey(key) => reduce_exit_confirm_key(state, key),
        Intent::ToggleHints => {
            state.toggle_hints_visibility();
            vec![Effect::RequestRender]
        }
        Intent::Tick => collect_effects(state.tick_all_nodes()),
        Intent::Noop
        | Intent::ScrollUp
        | Intent::ScrollDown
        | Intent::ScrollPageUp
        | Intent::ScrollPageDown
        | Intent::CopySelection
        | Intent::Pointer(_) => vec![],
        Intent::PointerOn { .. }
        | Intent::Back
        | Intent::Submit
        | Intent::ToggleCompletion
        | Intent::CompleteNext
        | Intent::CompletePrev
        | Intent::NextFocus
        | Intent::PrevFocus
        | Intent::TextAction(_)
        | Intent::OpenOverlay(_)
        | Intent::OpenOverlayAtIndex(_)
        | Intent::OpenOverlayShortcut
        | Intent::CloseOverlay => {
            vec![Effect::RequestRender]
        }
    }
}

fn reduce_exit_confirm_key(state: &mut AppState, key: KeyEvent) -> Vec<Effect> {
    match key.code {
        KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
            if state.toggle_exit_confirm_choice() {
                vec![Effect::RequestRender]
            } else {
                vec![]
            }
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if state.set_exit_confirm_choice(ExitConfirmChoice::Exit) {
                vec![Effect::RequestRender]
            } else {
                vec![]
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if state.set_exit_confirm_choice(ExitConfirmChoice::Stay) {
                vec![Effect::RequestRender]
            } else {
                vec![]
            }
        }
        KeyCode::Enter => {
            if state.resolve_exit_confirm() {
                vec![Effect::RequestRender]
            } else {
                vec![]
            }
        }
        KeyCode::Esc => {
            state.cancel_exit_confirm();
            vec![Effect::RequestRender]
        }
        _ => vec![],
    }
}

fn collect_effects(result: InteractionResult) -> Vec<Effect> {
    let mut effects: Vec<Effect> = result.actions.into_iter().map(Effect::Action).collect();
    if result.request_render {
        effects.push(Effect::RequestRender);
    }
    effects
}

fn is_plain_enter(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE
}

fn fallback_submit(state: &mut AppState) -> Vec<Effect> {
    if state.back_confirm().is_some() {
        state.confirm_back();
        return vec![Effect::RequestRender];
    }
    let effects = collect_effects(state.handle_system_event(SystemEvent::RequestSubmit));
    if effects.is_empty() {
        vec![Effect::RequestRender]
    } else {
        effects
    }
}
