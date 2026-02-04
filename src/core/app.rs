use crate::array_input::ArrayInput;
use crate::button_input::ButtonInput;
use crate::checkbox_input::CheckboxInput;
use crate::choice_input::ChoiceInput;
use crate::color_input::ColorInput;
use crate::components::select_component::{SelectComponent, SelectMode};
use crate::core::action_bindings::ActionBindings;
use crate::core::binding::{BindTarget, ValueSource};
use crate::core::event::Action;
use crate::core::event_queue::{AppEvent, EventQueue};
use crate::core::flow::{Flow, StepStatus};
use crate::core::layer_manager::LayerManager;
use crate::core::node::{Node, NodeId};
use crate::core::overlay::OverlayState;
use crate::core::reducer::{Effect, Reducer};
use crate::core::state::AppState;
use crate::core::step_builder::StepBuilder;
use crate::core::value::Value;
use crate::password_input::{PasswordInput, PasswordRender};
use crate::path_input::PathInput;
use crate::segmented_input::SegmentedInput;
use crate::select_input::SelectInput;
use crate::slider_input::SliderInput;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers, Terminal};
use crate::text_input::TextInput;
use crate::ui::render::{RenderOptions, RenderPipeline};
use crate::ui::theme::Theme;
use crate::validators;
use std::io;
use std::time::{Duration, Instant};

const ERROR_TIMEOUT: Duration = Duration::from_secs(2);
pub struct App {
    state: AppState,
    pipeline: RenderPipeline,
    bindings: ActionBindings,
    events: EventQueue,
    theme: Theme,

    last_rendered_step: usize,
    last_cursor: Option<(u16, u16)>,

    layer_manager: LayerManager,
    pending_layer_clear: bool,
}

impl App {
    pub fn new() -> Self {
        let flow = Flow::new(build_demo_steps());
        let state = AppState::new(flow);

        let mut pipeline = RenderPipeline::new();
        pipeline.set_decoration(true);
        pipeline.set_title("Steply");

        Self {
            state,
            pipeline,
            bindings: ActionBindings::new(),
            events: EventQueue::new(),
            theme: Theme::default(),
            last_rendered_step: 0,
            last_cursor: None,
            layer_manager: LayerManager::new(),
            pending_layer_clear: false,
        }
    }

    pub fn tick(&mut self) -> bool {
        let mut processed = false;
        let now = Instant::now();

        while let Some(event) = self.events.next_ready(now) {
            self.dispatch(event);
            processed = true;
        }

        processed
    }

    pub fn render(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        self.sync_step_bindings();

        self.pipeline.render_title(terminal, &self.theme)?;
        terminal.queue_hide_cursor()?;

        let current = self.state.flow.current_index();
        if current != self.last_rendered_step {
            self.render_completed_step(terminal)?;
            self.last_rendered_step = current;
        }

        if self.pending_layer_clear {
            self.pipeline.clear_layer(terminal)?;
            self.pending_layer_clear = false;
        }

        let step = self.state.flow.current_step();
        let registry = self.state.flow.registry();
        let options = RenderOptions::active();

        let step_cursor =
            self.pipeline
                .render_step(terminal, step, registry, &self.theme, options)?;

        let cursor = if let Some(overlay) = self.layer_manager.active() {
            let registry = self.state.flow.registry();
            self.pipeline
                .render_layer(terminal, overlay, registry, &self.theme, step_cursor)?
        } else {
            step_cursor
        };

        if let Some((col, row)) = cursor {
            self.last_cursor = Some((col, row));
            terminal.queue_move_cursor(col, row)?;
            terminal.queue_show_cursor()?;
        } else {
            self.last_cursor = None;
            terminal.queue_hide_cursor()?;
        }
        terminal.flush()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.events.emit(AppEvent::Key(key));
    }

    pub fn should_exit(&self) -> bool {
        self.state.should_exit
    }

    pub fn request_rerender(&mut self) {
        self.events.emit(AppEvent::RequestRerender);
    }

    pub fn move_to_end(&self, terminal: &mut Terminal) -> io::Result<()> {
        self.pipeline.move_to_end(terminal)
    }

    fn dispatch(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key_event(key),
            AppEvent::Action(action) => self.handle_action(action),
            AppEvent::ValueRequested { source, target } => {
                self.handle_value_requested(source, target);
            }
            AppEvent::ValueProduced { target, value, .. } => {
                self.handle_value_produced(target, value);
            }
            AppEvent::RequestRerender
            | AppEvent::InputChanged { .. }
            | AppEvent::FocusChanged { .. }
            | AppEvent::Submitted => {}
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('o') && key.modifiers == KeyModifiers::CONTROL {
            self.toggle_overlay();
            return;
        }

        if key.code == KeyCode::Esc && self.layer_manager.is_active() {
            self.close_overlay();
            return;
        }

        if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
            let registry = self.state.flow.registry_mut();
            if self.state.engine.handle_tab_completion(registry) {
                return;
            }
            self.handle_action(Action::NextInput);
            return;
        }

        if key.code == KeyCode::Enter
            && key.modifiers == KeyModifiers::NONE
            && self.state.engine.focused_target().is_none()
        {
            self.handle_action(Action::Submit);
            return;
        }

        if self.try_handle_component_key(key) {
            self.events.emit(AppEvent::RequestRerender);
            return;
        }

        let registry = self.state.flow.registry();
        let captured = self
            .state
            .engine
            .focused_caps(registry)
            .map(|caps| caps.captures_key(key.code, key.modifiers))
            .unwrap_or(false);

        if !captured {
            if let Some(action) = self.bindings.handle_key(&key) {
                self.handle_action(action);
                return;
            }
        }

        self.handle_action(Action::InputKey(key));
    }

    fn handle_action(&mut self, action: Action) {
        let effects = Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT);
        self.apply_effects(effects);
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Emit(event) => self.events.emit(event),
                Effect::EmitAfter(event, delay) => self.events.emit_after(event, delay),
                Effect::CancelClearError(id) => self.events.cancel_clear_error_message(&id),
            }
        }
    }

    fn current_step_node_ids(&self) -> Vec<NodeId> {
        let step = self.state.flow.current_step();
        step.node_ids.clone()
    }

    fn try_handle_component_key(&mut self, key: KeyEvent) -> bool {
        let Some(component_id) = self.state.engine.focused_component_id().cloned() else {
            return false;
        };

        let registry = self.state.flow.registry_mut();
        let Some(node) = registry.get_mut(&component_id) else {
            return false;
        };

        if let Node::Component(component) = node {
            let response = component.handle_key(key.code, key.modifiers);
            if let Some(value) = response.produced {
                if let Some(target) = component.bind_target() {
                    self.events.emit(AppEvent::ValueProduced {
                        source: ValueSource::Component(component_id.clone()),
                        target,
                        value,
                    });
                }
            }
            return response.handled;
        }

        false
    }

    fn sync_step_bindings(&mut self) {
        let node_ids = self.current_step_node_ids();
        let mut requests = Vec::new();
        {
            let registry = self.state.flow.registry();
            for id in &node_ids {
                if let Some(node) = registry.get(id) {
                    if let Node::Component(component) = node {
                        if let Some(target) = component.bind_target() {
                            requests.push((id.clone(), target));
                        }
                    }
                }
            }
        }

        for (component_id, target) in requests {
            self.handle_value_requested(ValueSource::Component(component_id), target);
        }
    }

    fn toggle_overlay(&mut self) {
        if self.layer_manager.is_active() {
            self.close_overlay();
        } else {
            self.open_overlay();
        }
    }

    fn open_overlay(&mut self) {
        let registry = self.state.flow.registry_mut();
        let engine = &mut self.state.engine;
        self.layer_manager
            .open(Box::new(OverlayState::demo()), registry, engine);

        if let Some(active) = self.layer_manager.active() {
            if let Some(target) = active.layer.bind_target() {
                self.events.emit(AppEvent::ValueRequested {
                    source: ValueSource::Layer(active.layer.id().to_string()),
                    target,
                });
            }
        }
    }

    fn close_overlay(&mut self) {
        let step_node_ids = self.current_step_node_ids();
        let registry = self.state.flow.registry_mut();
        let engine = &mut self.state.engine;
        if self
            .layer_manager
            .close(registry, engine, step_node_ids, &mut |event| {
                self.events.emit(event)
            })
        {
            self.pending_layer_clear = true;
        }
    }

    fn handle_value_requested(&mut self, source: ValueSource, target: BindTarget) {
        let value = {
            let registry = self.state.flow.registry();
            registry.get_value(&target)
        };

        let Some(value) = value else {
            return;
        };

        match source {
            ValueSource::Component(id) => {
                let registry = self.state.flow.registry_mut();
                if let Some(component) = registry.get_component_mut(&id) {
                    component.set_value(value);
                }
            }
            ValueSource::Layer(id) => {
                let registry = self.state.flow.registry_mut();
                if let Some(active) = self.layer_manager.active_mut() {
                    if active.layer.id() == id {
                        active.layer.set_value(registry, value);
                    }
                }
            }
        }
    }

    fn handle_value_produced(&mut self, target: BindTarget, value: Value) {
        let registry = self.state.flow.registry_mut();
        registry.set_value(&target, value);
    }

    fn render_completed_step(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        let prev_index = self.last_rendered_step;
        let Some(step) = self.state.flow.step_at(prev_index) else {
            return Ok(());
        };

        let registry = self.state.flow.registry();
        let options = RenderOptions::done();

        self.pipeline
            .render_step(terminal, step, registry, &self.theme, options)?;
        self.pipeline.move_to_end(terminal)?;
        self.pipeline
            .write_connector(terminal, &self.theme, StepStatus::Done, 1)?;
        self.pipeline.reset_region();

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn build_demo_steps() -> Vec<(crate::core::step::Step, Vec<(NodeId, Node)>)> {
    vec![
        build_step_zero(),
        build_step_one(),
        build_step_two(),
        build_step_three(),
    ]
}

fn build_step_zero() -> (crate::core::step::Step, Vec<(NodeId, Node)>) {
    let component = SelectComponent::new(
        "plan_select",
        vec!["Free".to_string(), "Pro".to_string(), "Team".to_string()],
    )
    .with_label("Select plan:")
    .with_mode(SelectMode::List);

    let tags_component = SelectComponent::new("tags_select", Vec::new())
        .with_label("Tags (from input):")
        .with_mode(SelectMode::Multi)
        .bind_to_input("tags");

    StepBuilder::new("Component demo:")
        .input(ArrayInput::new("tags", "Tags"))
        .input(ButtonInput::new("cta", "Continue").with_text("Continue"))
        .component(tags_component)
        .input(TextInput::new("username", "Username"))
        .component(component)
        .build()
}

fn build_step_one() -> (crate::core::step::Step, Vec<(NodeId, Node)>) {
    let tags_component = SelectComponent::new("tags_select", Vec::new())
        .with_label("Tags (from input):")
        .with_mode(SelectMode::Multi)
        .bind_to_input("tags");

    StepBuilder::new("Please fill the form:")
        .hint("Press Tab/Shift+Tab to navigate, Enter to submit, Esc to exit")
        .input(
            TextInput::new("username", "Username")
                .with_validator(validators::required())
                .with_validator(validators::min_length(3)),
        )
        .input(
            TextInput::new("email", "Email")
                .with_validator(validators::required())
                .with_validator(validators::email()),
        )
        .input(ColorInput::new("accent_color", "Accent Color").with_rgb(64, 120, 200))
        .input(CheckboxInput::new("tos", "Accept Terms").with_checked(true))
        .input(
            ChoiceInput::new(
                "plan",
                "Plan",
                vec!["Free".to_string(), "Pro".to_string(), "Team".to_string()],
            )
            .with_bullets(true),
        )
        .input(ArrayInput::new("tags", "Tags"))
        .component(tags_component)
        .input(PathInput::new("path", "Path"))
        .input(SelectInput::new(
            "color",
            "Color",
            vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
        ))
        .build()
}

fn build_step_two() -> (crate::core::step::Step, Vec<(NodeId, Node)>) {
    StepBuilder::new("Almost there:")
        .input(
            TextInput::new("password", "Password")
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )
        .build()
}

fn build_step_three() -> (crate::core::step::Step, Vec<(NodeId, Node)>) {
    StepBuilder::new("Final step:")
        .hint("Try arrows left/right in select, and masked input")
        .input(
            PasswordInput::new("new_password", "New Password")
                .with_render_mode(PasswordRender::Stars)
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )
        .input(SliderInput::new("volume", "Volume", 1, 20))
        .input(SegmentedInput::ipv4("ip_address", "IP Address"))
        .input(SegmentedInput::phone_us("phone", "Phone"))
        .input(SegmentedInput::number("num", "Number"))
        .input(SegmentedInput::date_dd_mm_yyyy("birthdate", "Birth Date"))
        .build()
}
