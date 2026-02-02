use crate::action_bindings::ActionBindings;
use crate::date_input::DateTimeInput;
use crate::event::Action;
use crate::event_queue::{AppEvent, EventQueue};
use crate::node::Node;
use crate::reducer::{Effect, Reducer};
use crate::renderer::Renderer;
use crate::state::AppState;
use crate::step::Step;
use crate::terminal::KeyEvent;
use crate::terminal::Terminal;
use crate::text_input::TextInput;
use crate::theme::Theme;
use crate::validators;
use std::io;
use std::time::{Duration, Instant};

const ERROR_TIMEOUT: Duration = Duration::from_secs(2);

pub struct App {
    pub state: AppState,
    pub renderer: Renderer,
    action_bindings: ActionBindings,
    event_queue: EventQueue,
    theme: Theme,
}

impl App {
    pub fn new() -> Self {
        let app = Self {
            state: AppState::new(build_step()),
            renderer: Renderer::new(),
            action_bindings: ActionBindings::new(),
            event_queue: EventQueue::new(),
            theme: Theme::default_theme(),
        };

        app
    }

    pub fn tick(&mut self) -> bool {
        let mut processed_any = false;
        loop {
            let now = Instant::now();
            let Some(event) = self.event_queue.next_ready(now) else {
                break;
            };
            self.dispatch_event(event);
            processed_any = true;
        }
        processed_any
    }

    pub fn render(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        self.renderer.render(
            &self.state.engine.step,
            &self.state.view,
            &self.theme,
            terminal,
        )
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) {
        self.event_queue.emit(AppEvent::Key(key_event));
    }

    pub fn should_exit(&self) -> bool {
        self.state.should_exit
    }

    fn dispatch_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key_event) => {
                let captured = self
                    .state
                    .engine
                    .focused_input_caps()
                    .map(|caps| caps.captures_key(key_event.code, key_event.modifiers))
                    .unwrap_or(false);

                if !captured {
                    if let Some(action) = self.action_bindings.handle_key(&key_event) {
                        let effects = Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT);
                        self.apply_effects(effects);
                        return;
                    }
                }

                let effects =
                    Reducer::reduce(&mut self.state, Action::InputKey(key_event), ERROR_TIMEOUT);
                self.apply_effects(effects);
            }
            AppEvent::Action(action) => {
                let effects = Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT);
                self.apply_effects(effects);
            }
            AppEvent::InputChanged { .. } | AppEvent::FocusChanged { .. } | AppEvent::Submitted => {
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Emit(event) => self.event_queue.emit(event),
                Effect::EmitAfter(event, delay) => self.event_queue.emit_after(event, delay),
                Effect::CancelClearError(id) => self.event_queue.cancel_clear_error_message(&id),
            }
        }
    }
}

fn build_step() -> Step {
    Step {
        prompt: "Please fill the form:".to_string(),
        hint: Some("Press Tab/Shift+Tab to navigate, Enter to submit, Esc to exit".to_string()),
        nodes: vec![
            Node::input(
                TextInput::new("username", "Username")
                    .with_validator(validators::required())
                    .with_validator(validators::min_length(3)),
            ),
            Node::input(
                TextInput::new("email", "Email")
                    .with_validator(validators::required())
                    .with_validator(validators::email()),
            ),
            Node::input(
                TextInput::new("password", "Password")
                    .with_validator(validators::required())
                    .with_validator(validators::min_length(8)),
            ),
            Node::input(DateTimeInput::new("birthdate", "Birth Date", "DD/MM/YYYY")),
            Node::input(DateTimeInput::new("meeting_time", "Meeting Time", "HH:mm")),
        ],
        form_validators: Vec::new(),
    }
}
