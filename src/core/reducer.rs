use crate::core::effect::Effect;
use crate::runtime::command::Command;
use crate::runtime::event::WidgetEvent;
use crate::state::app_state::AppState;

pub struct Reducer;

impl Reducer {
    pub fn reduce(state: &mut AppState, command: Command) -> Vec<Effect> {
        let mut effects = match command {
            Command::Exit => {
                if state.has_active_overlay() {
                    state.close_overlay();
                } else {
                    state.request_exit();
                }
                vec![Effect::RequestRender]
            }
            Command::Submit => {
                if let Some(result) = state.submit_focused() {
                    Self::effects_from_widget_events(result.events)
                } else {
                    vec![Effect::RequestRender]
                }
            }
            Command::NextFocus => {
                state.focus_next();
                vec![Effect::RequestRender]
            }
            Command::PrevFocus => {
                state.focus_prev();
                vec![Effect::RequestRender]
            }
            Command::InputKey(key) => {
                let result = state.dispatch_key_to_focused(key);
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.handled {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Command::TextAction(action) => {
                let result = state.dispatch_text_action_to_focused(action);
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.handled {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Command::OpenOverlay(overlay_id) => {
                vec![Effect::EmitWidget(WidgetEvent::OpenOverlay { overlay_id })]
            }
            Command::CloseOverlay => vec![Effect::EmitWidget(WidgetEvent::CloseOverlay)],
            Command::Tick => {
                let result = state.tick_all_nodes();
                let mut effects = Self::effects_from_widget_events(result.events);
                if result.handled {
                    effects.push(Effect::RequestRender);
                }
                effects
            }
            Command::Noop => vec![],
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
