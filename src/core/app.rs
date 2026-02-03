use crate::action_bindings::ActionBindings;
use crate::array_input::ArrayInput;
use crate::checkbox_input::CheckboxInput;
use crate::choice_input::ChoiceInput;
use crate::color_input::ColorInput;
use crate::event::Action;
use crate::event_queue::{AppEvent, EventQueue};
use crate::flow::Flow;
use crate::node::Node;
use crate::password_input::{PasswordInput, PasswordRender};
use crate::reducer::{Effect, Reducer};
use crate::renderer::Renderer;
use crate::segmented_input::SegmentedInput;
use crate::select_input::SelectInput;
use crate::slider_input::SliderInput;
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
const ENABLE_DECORATION: bool = true;
const APP_TITLE: &str = "Steply main";

pub struct App {
    pub state: AppState,
    pub renderer: Renderer,
    action_bindings: ActionBindings,
    event_queue: EventQueue,
    theme: Theme,
    last_rendered_step: usize,
}

impl App {
    pub fn new() -> Self {
        let flow = Flow::new(build_steps());
        let mut app = Self {
            state: AppState::new(flow),
            renderer: Renderer::new(),
            action_bindings: ActionBindings::new(),
            event_queue: EventQueue::new(),
            theme: Theme::default_theme(),
            last_rendered_step: 0,
        };

        app.renderer.set_decoration_enabled(ENABLE_DECORATION);
        app.renderer.set_title(APP_TITLE);
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
        self.renderer.render_title_once(terminal, &self.theme)?;

        let current_step = self.state.flow.current_index();
        if current_step != self.last_rendered_step {
            if let Some(step) = self.state.flow.step_at(self.last_rendered_step) {
                let done_view = crate::view_state::ViewState::new();
                self.renderer.render_with_status(
                    step,
                    &done_view,
                    &self.theme,
                    terminal,
                    crate::flow::StepStatus::Done,
                    true,
                )?;
            }
            self.renderer.move_to_end(terminal)?;
            self.renderer.write_connector_lines(
                terminal,
                &self.theme,
                crate::flow::StepStatus::Done,
                1,
            )?;
            self.renderer.reset_block();
            self.last_rendered_step = current_step;
        }

        self.renderer.render_with_status(
            self.state.flow.current_step(),
            &self.state.view,
            &self.theme,
            terminal,
            self.state.flow.current_status(),
            false,
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
                    .focused_input_caps(self.state.flow.current_step())
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
            Node::input(ColorInput::new("accent_color", "Accent Color").with_rgb(64, 120, 200)),
            Node::input(CheckboxInput::new("tos", "Accept Terms").with_checked(true)),
            Node::input(
                ChoiceInput::new(
                    "plan",
                    "Plan",
                    vec!["Free".to_string(), "Pro".to_string(), "Team".to_string()],
                )
                .with_bullets(true),
            ),
            Node::input(ArrayInput::new("tags", "Tags")),
            Node::input(SelectInput::new(
                "color",
                "Color",
                vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            )),
        ],
        form_validators: Vec::new(),
    }
}

fn build_step_two() -> Step {
    Step {
        prompt: "Almost there:".to_string(),
        hint: None,
        nodes: vec![Node::input(
            TextInput::new("password", "Password")
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )],
        form_validators: Vec::new(),
    }
}

fn build_step_three() -> Step {
    Step {
        prompt: "Final step:".to_string(),
        hint: Some("Try arrows left/right in select, and masked input".to_string()),
        nodes: vec![
            Node::input(
                PasswordInput::new("new_password", "New Password")
                    .with_render_mode(PasswordRender::Stars)
                    .with_validator(validators::required())
                    .with_validator(validators::min_length(8)),
            ),
            Node::input(SliderInput::new("email", "Email", 1, 20)),
            Node::input(SegmentedInput::ipv4("ip_address", "IP Address")),
            Node::input(SegmentedInput::phone_us("phone", "Phone")),
            Node::input(SegmentedInput::number("num", "num")),
            Node::input(SegmentedInput::date_dd_mm_yyyy(
                "birthdate_masked",
                "Birth Date",
            )),
        ],
        form_validators: Vec::new(),
    }
}

fn build_steps() -> Vec<Step> {
    vec![build_step(), build_step_two(), build_step_three()]
}
