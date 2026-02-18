use crate::runtime::effect::Effect;
use crate::runtime::event::SystemEvent;
use crate::runtime::intent::Intent;
use crate::state::app::AppState;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::traits::InteractionResult;

pub struct Reducer;

impl Reducer {
    pub fn reduce(state: &mut AppState, intent: Intent) -> Vec<Effect> {
        let mut effects = match intent {
            Intent::Exit => {
                state.request_exit();
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
                    // Esc first closes the completion menu before closing overlays or exiting.
                    state.suppress_completion_tab_for_focused();
                } else if state
                    .dispatch_key_to_focused(KeyEvent {
                        code: KeyCode::Esc,
                        modifiers: KeyModifiers::NONE,
                    })
                    .handled
                {
                    // Focused widget consumed Esc (e.g. file browser clears query).
                } else if state.has_active_overlay() {
                    state.close_overlay();
                }
                vec![Effect::RequestRender]
            }
            Intent::Submit => {
                if state.back_confirm().is_some() {
                    state.confirm_back();
                    return vec![Effect::RequestRender];
                }
                if let Some(result) = state.submit_focused() {
                    collect_effects(result)
                } else {
                    vec![Effect::RequestRender]
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
            Intent::InputKey(key) => collect_effects(state.dispatch_key_to_focused(key)),
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
            Intent::Tick => collect_effects(state.tick_all_nodes()),
            Intent::Noop => vec![],
            Intent::ScrollUp
            | Intent::ScrollDown
            | Intent::ScrollPageUp
            | Intent::ScrollPageDown => vec![],
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

fn collect_effects(result: InteractionResult) -> Vec<Effect> {
    let mut effects: Vec<Effect> = result.actions.into_iter().map(Effect::Action).collect();
    if result.request_render {
        effects.push(Effect::RequestRender);
    }
    effects
}
