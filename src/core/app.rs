use crate::array_input::ArrayInput;
use crate::button_input::ButtonInput;
use crate::checkbox_input::CheckboxInput;
use crate::choice_input::ChoiceInput;
use crate::color_input::ColorInput;
use crate::components::file_browser::{FileBrowserState, overlay_for_list};
use crate::components::select_component::{SelectComponent, SelectMode};
use crate::core::action_bindings::ActionBindings;
use crate::core::binding::{BindTarget, ValueSource};
use crate::core::event::Action;
use crate::core::event_queue::{AppEvent, EventQueue};
use crate::core::flow::{Flow, StepStatus};
use crate::core::layer_manager::LayerManager;
use crate::core::node::{
    Node, find_component, find_component_mut, find_input, find_input_mut, poll_components,
};
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
use std::sync::{Arc, Mutex};
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
    file_browser_overlay_state: Option<Arc<Mutex<FileBrowserState>>>,
}

impl App {
    pub fn new() -> Self {
        let (steps, file_browser_overlay_state) = build_demo_steps();
        let flow = Flow::new(steps);
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
            file_browser_overlay_state,
        }
    }

    pub fn tick(&mut self) -> bool {
        let mut processed = false;
        let now = Instant::now();

        while let Some(event) = self.events.next_ready(now) {
            self.dispatch(event);
            processed = true;
        }

        let polled = if let Some(active) = self.layer_manager.active_mut() {
            if active.layer.focus_mode() == crate::core::layer::LayerFocusMode::Modal {
                poll_components(active.nodes_mut())
            } else {
                let nodes = self.state.flow.current_step_mut().nodes.as_mut_slice();
                let step_polled = poll_components(nodes);
                let overlay_polled = poll_components(active.nodes_mut());
                step_polled || overlay_polled
            }
        } else {
            let nodes = self.state.flow.current_step_mut().nodes.as_mut_slice();
            poll_components(nodes)
        };
        if polled {
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
        let mut options = RenderOptions::active();
        if self.layer_manager.is_active() {
            options.connect_to_next = true;
        }

        let step_cursor = self
            .pipeline
            .render_step(terminal, step, &self.theme, options)?;

        let cursor = if let Some(overlay) = self.layer_manager.active() {
            let overlay_cursor =
                self.pipeline
                    .render_layer(terminal, overlay, &self.theme, step_cursor)?;
            if overlay.layer.focus_mode() == crate::core::layer::LayerFocusMode::Shared {
                step_cursor
            } else {
                overlay_cursor
            }
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
            AppEvent::ValueProduced {
                source,
                target,
                value,
            } => {
                self.handle_value_produced(source, target, value);
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

        if !self.layer_manager.is_active() && self.should_open_file_browser_overlay(&key) {
            self.open_overlay();
        }

        if let Some(active) = self.layer_manager.active_mut() {
            let mut emit = |event| self.events.emit(event);
            if active.layer.handle_key(key, &mut emit) {
                return;
            }
        }

        if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
            self.handle_action(Action::TabKey(key));
            return;
        }

        if key.code == KeyCode::Enter
            && key.modifiers == KeyModifiers::NONE
            && self.state.engine.focused_node_id().is_none()
        {
            self.handle_action(Action::Submit);
            return;
        }

        if let Some(action) = self.bindings.handle_key(&key) {
            self.handle_action(action);
            return;
        }

        self.handle_action(Action::InputKey(key));
    }

    fn handle_action(&mut self, action: Action) {
        let effects = if let Some(active) = self.layer_manager.active_mut() {
            if active.layer.focus_mode() == crate::core::layer::LayerFocusMode::Modal {
                Reducer::reduce(
                    &mut self.state,
                    action,
                    ERROR_TIMEOUT,
                    Some(active.nodes_mut()),
                )
            } else {
                Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT, None)
            }
        } else {
            Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT, None)
        };
        self.apply_effects(effects);
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Emit(event) => self.events.emit(event),
                Effect::EmitAfter(event, delay) => self.events.emit_after(event, delay),
                Effect::CancelClearError(id) => self.events.cancel_clear_error_message(&id),
                Effect::ComponentProduced { id, value } => {
                    self.handle_component_produced(&id, value);
                }
            }
        }
    }

    fn active_nodes(&self) -> &[Node] {
        if let Some(active) = self.layer_manager.active() {
            return active.nodes();
        }
        self.state.flow.current_step().nodes.as_slice()
    }

    fn handle_component_produced(&mut self, id: &str, value: Value) {
        let Some(component) = find_component(self.active_nodes(), id) else {
            return;
        };
        let Some(target) = component.bind_target() else {
            return;
        };
        self.events.emit(AppEvent::ValueProduced {
            source: ValueSource::Component(id.to_string()),
            target,
            value,
        });
    }

    fn value_for_target(&self, target: &BindTarget) -> Option<Value> {
        match target {
            BindTarget::Input(id) => self.find_input_any(id).map(|input| input.value_typed()),
            BindTarget::Component(id) => self
                .find_component_any(id)
                .and_then(|component| component.value()),
        }
    }

    fn set_value_for_target(&mut self, target: &BindTarget, value: Value) {
        match target {
            BindTarget::Input(id) => {
                if let Some(input) = self.find_input_mut_any(id) {
                    input.set_value_typed(value);
                }
            }
            BindTarget::Component(id) => {
                if let Some(component) = self.find_component_mut_any(id) {
                    component.set_value(value);
                }
            }
        }
    }

    fn find_input_any(&self, id: &str) -> Option<&dyn crate::inputs::Input> {
        find_input(self.active_nodes(), id).or_else(|| {
            let step_nodes = self.state.flow.current_step().nodes.as_slice();
            find_input(step_nodes, id)
        })
    }

    fn find_input_mut_any(&mut self, id: &str) -> Option<&mut dyn crate::inputs::Input> {
        if let Some(active) = self.layer_manager.active_mut() {
            if let Some(input) = find_input_mut(active.nodes_mut(), id) {
                return Some(input);
            }
        }
        find_input_mut(self.state.flow.current_step_mut().nodes.as_mut_slice(), id)
    }

    fn find_component_any(&self, id: &str) -> Option<&dyn crate::core::component::Component> {
        find_component(self.active_nodes(), id).or_else(|| {
            let step_nodes = self.state.flow.current_step().nodes.as_slice();
            find_component(step_nodes, id)
        })
    }

    fn find_component_mut_any(
        &mut self,
        id: &str,
    ) -> Option<&mut dyn crate::core::component::Component> {
        if let Some(active) = self.layer_manager.active_mut() {
            if let Some(component) = find_component_mut(active.nodes_mut(), id) {
                return Some(component);
            }
        }
        find_component_mut(self.state.flow.current_step_mut().nodes.as_mut_slice(), id)
    }

    fn sync_step_bindings(&mut self) {
        let mut requests = Vec::new();
        collect_component_bindings(
            self.state.flow.current_step().nodes.as_slice(),
            &mut requests,
        );

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
        let engine = &mut self.state.engine;
        let step_nodes = self.state.flow.current_step_mut().nodes.as_mut_slice();
        if let Some(state) = self.file_browser_overlay_state.clone() {
            let overlay = overlay_for_list("file_browser_overlay", "", state);
            self.layer_manager
                .open(Box::new(overlay), step_nodes, engine);
        } else {
            self.layer_manager
                .open(Box::new(OverlayState::demo()), step_nodes, engine);
        }

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
        let engine = &mut self.state.engine;
        if self.layer_manager.close(
            engine,
            self.state.flow.current_step_mut().nodes.as_mut_slice(),
            &mut |event| {
                self.events.emit(event);
            },
        ) {
            self.pending_layer_clear = true;
        }
    }

    fn handle_value_requested(&mut self, source: ValueSource, target: BindTarget) {
        let Some(value) = self.value_for_target(&target) else {
            return;
        };

        match source {
            ValueSource::Component(id) => {
                if let Some(component) = self.find_component_mut_any(&id) {
                    component.set_value(value);
                }
            }
            ValueSource::Layer(id) => {
                if let Some(active) = self.layer_manager.active_mut() {
                    if active.layer.id() == id {
                        active.layer.set_value(value);
                    }
                }
            }
        }
    }

    fn handle_value_produced(&mut self, source: ValueSource, target: BindTarget, value: Value) {
        self.set_value_for_target(&target, value);
        if let ValueSource::Layer(id) = source {
            if let Some(active) = self.layer_manager.active() {
                if active.layer.id() == id {
                    self.close_overlay();
                }
            }
        }
    }

    fn render_completed_step(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        let prev_index = self.last_rendered_step;
        let Some(step) = self.state.flow.step_at(prev_index) else {
            return Ok(());
        };

        let options = RenderOptions::done();

        self.pipeline
            .render_step(terminal, step, &self.theme, options)?;
        self.pipeline.move_to_end(terminal)?;
        self.pipeline
            .write_connector(terminal, &self.theme, StepStatus::Done, 1)?;
        self.pipeline.reset_region();

        Ok(())
    }

    fn should_open_file_browser_overlay(&self, key: &KeyEvent) -> bool {
        let Some(state) = self.file_browser_overlay_state.as_ref() else {
            return false;
        };
        let edits_input = matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
        );
        if !edits_input {
            return false;
        }
        let focused = self.state.engine.focused_node_id();
        let Ok(state) = state.lock() else {
            return false;
        };
        focused.map(|id| id == state.input_id()).unwrap_or(false)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_component_bindings(nodes: &[Node], out: &mut Vec<(String, BindTarget)>) {
    for node in nodes {
        match node {
            Node::Component(component) => {
                if let Some(target) = component.bind_target() {
                    out.push((component.id().to_string(), target));
                }
                if let Some(children) = component.children() {
                    collect_component_bindings(children, out);
                }
            }
            _ => {}
        }
    }
}

fn build_demo_steps() -> (
    Vec<crate::core::step::Step>,
    Option<Arc<Mutex<FileBrowserState>>>,
) {
    let file_browser_state = Arc::new(Mutex::new(FileBrowserState::new("plan_select")));
    let steps = vec![
        build_step_zero(file_browser_state.clone()),
        build_step_one(),
        build_step_two(),
        build_step_three(),
    ];
    (steps, Some(file_browser_state))
}

fn build_step_zero(file_browser_state: Arc<Mutex<FileBrowserState>>) -> crate::core::step::Step {
    let component =
        crate::components::file_browser::FileBrowserInputComponent::from_state(file_browser_state)
            .with_label("Select plan:")
            .with_recursive_search(true)
            .with_max_visible(6)
            // .with_entry_filter(crate::components::file_browser::EntryFilter::FilesOnly)
            // .with_extension_filter([".yml", ".yaml"])
            .with_relative_paths(true)
            .with_placeholder("Type to filter");

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

fn build_step_one() -> crate::core::step::Step {
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

fn build_step_two() -> crate::core::step::Step {
    StepBuilder::new("Almost there:")
        .input(
            TextInput::new("password", "Password")
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )
        .build()
}

fn build_step_three() -> crate::core::step::Step {
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
