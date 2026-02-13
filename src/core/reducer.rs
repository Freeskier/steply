use crate::core::effect::Effect;
use crate::runtime::event::WidgetEvent;
use crate::runtime::intent::Intent;
use crate::state::app_state::AppState;

pub struct Reducer;

impl Reducer {
    pub fn reduce(state: &mut AppState, intent: Intent) -> Vec<Effect> {
        let mut effects = match intent {
            Intent::Exit => {
                state.request_exit();
                vec![Effect::RequestRender]
            }
            Intent::Cancel => {
                if state.cancel_completion_for_focused() {
                    // Esc first closes completion UI before closing overlays/exiting app.
                } else if state.has_active_overlay() {
                    state.close_overlay();
                } else {
                    state.request_exit();
                }
                vec![Effect::RequestRender]
            }
            Intent::Submit => {
                if let Some(result) = state.submit_focused() {
                    let mut effects = Self::effects_from_widget_events(result.events);
                    if result.request_render {
                        effects.push(Effect::RequestRender);
                    }
                    effects
                } else {
                    vec![Effect::RequestRender]
                }
            }
            Intent::NextFocus => {
                let result = state.handle_tab_forward();
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.request_render {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Intent::PrevFocus => {
                let result = state.handle_tab_backward();
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.request_render {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Intent::InputKey(key) => {
                let result = state.dispatch_key_to_focused(key);
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.request_render {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Intent::TextAction(action) => {
                let result = state.dispatch_text_action_to_focused(action);
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.request_render {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Intent::OpenOverlay(overlay_id) => {
                vec![Effect::EmitWidget(WidgetEvent::OpenOverlay { overlay_id })]
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
            Intent::CloseOverlay => vec![Effect::EmitWidget(WidgetEvent::CloseOverlay)],
            Intent::Tick => {
                let result = state.tick_all_nodes();
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.request_render {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Intent::Noop => vec![],
        };

        effects.extend(
            state
                .take_pending_scheduler_commands()
                .into_iter()
                .map(Effect::Schedule),
        );

        effects
    }

    fn effects_from_widget_events(events: Vec<WidgetEvent>) -> Vec<Effect> {
        events.into_iter().map(Effect::EmitWidget).collect()
    }
}
