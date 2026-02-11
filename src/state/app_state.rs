use crate::app::event::{AppEvent, WidgetEvent};
use crate::app::scheduler::SchedulerCommand;
use crate::domain::value::Value;
use crate::node::{Node, apply_focus, find_node_mut, visit_nodes, visit_nodes_mut};
use crate::state::flow::Flow;
use crate::state::focus::FocusState;
use crate::state::layer::{LayerManager, LayerMode, LayerState};
use crate::state::step::Step;
use crate::state::store::ValueStore;
use crate::state::validation::{ErrorVisibility, ValidationState};
use crate::terminal::terminal::KeyEvent;
use crate::widgets::inputs::input::Input;
use crate::widgets::outputs::text::Text;
use crate::widgets::traits::{InteractionResult, TextAction};
use std::collections::HashMap;
use std::time::Duration;

const ERROR_INLINE_TTL: Duration = Duration::from_secs(2);

pub struct AppState {
    flow: Flow,
    layers: LayerManager,
    store: ValueStore,
    validation: ValidationState,
    pending_scheduler: Vec<SchedulerCommand>,
    pub focus: FocusState,
    pub should_exit: bool,
}

impl AppState {
    pub fn new(flow: Flow) -> Self {
        let mut state = Self {
            flow,
            layers: LayerManager::new(),
            store: ValueStore::new(),
            validation: ValidationState::default(),
            pending_scheduler: Vec::new(),
            focus: FocusState::default(),
            should_exit: false,
        };
        state.rebuild_focus();
        state
    }

    pub fn current_step_id(&self) -> &str {
        &self.flow.current_step().id
    }

    pub fn current_step_index(&self) -> usize {
        self.flow.current_index()
    }

    pub fn steps(&self) -> &[Step] {
        self.flow.steps()
    }

    pub fn current_prompt(&self) -> &str {
        &self.flow.current_step().prompt
    }

    pub fn current_hint(&self) -> Option<&str> {
        self.flow.current_step().hint.as_deref()
    }

    pub fn active_nodes(&self) -> &[Node] {
        if let Some(layer) = self.layers.active() {
            return &layer.nodes;
        }
        &self.flow.current_step().nodes
    }

    pub fn active_nodes_mut(&mut self) -> &mut [Node] {
        if let Some(layer) = self.layers.active_mut() {
            return &mut layer.nodes;
        }
        &mut self.flow.current_step_mut().nodes
    }

    pub fn has_active_layer(&self) -> bool {
        self.layers.has_active()
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.validation.visible_error(id)
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.validation.is_hidden_invalid(id)
    }

    pub fn dispatch_key_to_focused(&mut self, key: KeyEvent) -> InteractionResult {
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        let result = {
            let nodes = self.active_nodes_mut();
            let Some(node) = find_node_mut(nodes, &focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_key(key)
        };

        if result.handled {
            // Live typing: keep validation updated, but do not reveal error yet.
            self.validate_focused(false);
        }
        self.apply_focus_state();
        result
    }

    pub fn dispatch_text_action_to_focused(&mut self, action: TextAction) -> InteractionResult {
        let Some(focused_id) = self.focus.current_id().map(ToOwned::to_owned) else {
            return InteractionResult::ignored();
        };

        let result = {
            let nodes = self.active_nodes_mut();
            let Some(node) = find_node_mut(nodes, &focused_id) else {
                return InteractionResult::ignored();
            };
            node.on_text_action(action)
        };

        if result.handled {
            // Global text actions should behave like typing: hidden until blur/submit.
            self.validate_focused(false);
        }
        self.apply_focus_state();
        result
    }

    pub fn submit_focused(&mut self) -> Option<InteractionResult> {
        let focused_id = self.focus.current_id()?.to_string();
        let nodes = self.active_nodes_mut();
        let node = find_node_mut(nodes, &focused_id)?;
        Some(node.on_event(&WidgetEvent::RequestSubmit))
    }

    pub fn tick_active_nodes(&mut self) -> InteractionResult {
        let mut merged = InteractionResult::ignored();

        for node in self.active_nodes_mut() {
            let result = node.on_tick();
            merged.handled |= result.handled;
            merged.events.extend(result.events);
        }
        merged
    }

    pub fn handle_widget_event(&mut self, event: WidgetEvent) {
        match event {
            WidgetEvent::ValueProduced { target, value } => {
                self.set_value_by_id(&target, value);
            }
            WidgetEvent::ClearInlineError { id } => {
                self.validation.clear_error(&id);
            }
            WidgetEvent::RequestSubmit => {
                if self.layers.has_active() {
                    self.close_layer();
                } else {
                    self.handle_step_submit();
                }
            }
            WidgetEvent::RequestFocus { target } => {
                self.focus.set_focus_by_id(&target);
                self.apply_focus_state();
            }
            WidgetEvent::OpenLayer { layer_id } => {
                self.open_demo_layer(layer_id);
            }
            WidgetEvent::CloseLayer => self.close_layer(),
            WidgetEvent::RequestRender => {}
        }
    }

    pub fn focus_next(&mut self) {
        self.validate_focused(false);
        self.focus.next();
        self.apply_focus_state();
    }

    pub fn focus_prev(&mut self) {
        self.validate_focused(false);
        self.focus.prev();
        self.apply_focus_state();
    }

    pub fn open_demo_layer(&mut self, layer_id: String) {
        let saved_focus_id = self.focus.current_id().map(ToOwned::to_owned);
        let overlay_input =
            Input::new("overlay_input", "Overlay input").with_submit_target("tags_raw".to_string());
        let nodes = vec![
            Node::Output(Box::new(Text::new(
                "overlay_label",
                "Overlay active. Enter copies value to tags_raw. Esc closes overlay.",
            ))),
            Node::Input(Box::new(overlay_input)),
        ];
        self.layers.open(
            LayerState::new(layer_id, LayerMode::Modal, nodes),
            saved_focus_id,
        );
        self.rebuild_focus();
    }

    pub fn close_layer(&mut self) {
        let restored_focus = self.layers.close();
        self.rebuild_focus_with_target(restored_focus.as_deref());
    }

    fn handle_step_submit(&mut self) {
        if !self.validate_current_step(true) {
            self.focus_first_invalid_on_current_step();
            self.apply_focus_state();
            return;
        }

        // Persist all visible node values from current step into global store.
        self.sync_current_step_values_to_store();

        if self.flow.next() {
            self.hydrate_current_step_from_store();
            self.rebuild_focus();
        } else {
            self.should_exit = true;
        }
    }

    fn sync_current_step_values_to_store(&mut self) {
        let values = {
            let mut out = Vec::<(String, Value)>::new();
            visit_nodes(self.flow.current_step().nodes.as_slice(), &mut |node| {
                if let Some(value) = node.value() {
                    out.push((node.id().to_string(), value));
                }
            });
            out
        };

        for (id, value) in values {
            self.set_value_by_id(&id, value);
        }
    }

    fn set_value_by_id(&mut self, id: &str, value: Value) {
        self.write_value_direct(id, value);
    }

    fn write_value_direct(&mut self, id: &str, value: Value) {
        self.store.set(id.to_string(), value.clone());

        if let Some(node) = find_node_mut(self.flow.current_step_mut().nodes.as_mut_slice(), id) {
            node.set_value(value.clone());
            if node.validate().is_ok() {
                self.validation.clear_error(id);
            }
        }

        if let Some(layer) = self.layers.active_mut() {
            if let Some(node) = find_node_mut(layer.nodes.as_mut_slice(), id) {
                node.set_value(value);
                if node.validate().is_ok() {
                    self.validation.clear_error(id);
                }
            }
        }
    }

    fn hydrate_current_step_from_store(&mut self) {
        let values: HashMap<String, Value> = self
            .store
            .iter()
            .map(|(id, value)| (id.to_string(), value.clone()))
            .collect();

        visit_nodes_mut(
            self.flow.current_step_mut().nodes.as_mut_slice(),
            &mut |node| {
                if let Some(value) = values.get(node.id()) {
                    node.set_value(value.clone());
                }
            },
        );
    }

    fn rebuild_focus_with_target(&mut self, target: Option<&str>) {
        self.focus = FocusState::from_nodes(self.active_nodes());
        if let Some(id) = target {
            self.focus.set_focus_by_id(id);
            if self.focus.current_id().is_none() {
                self.focus = FocusState::from_nodes(self.active_nodes());
            }
        }
        self.prune_validation_for_active_nodes();
        self.apply_focus_state();
    }

    fn rebuild_focus(&mut self) {
        self.rebuild_focus_with_target(None);
    }

    fn apply_focus_state(&mut self) {
        let focused = self.focus.current_id().map(ToOwned::to_owned);
        apply_focus(self.active_nodes_mut(), focused.as_deref());
    }

    fn validate_focused(&mut self, reveal: bool) -> bool {
        let Some(id) = self.focus.current_id().map(ToOwned::to_owned) else {
            return true;
        };
        self.validate_in_active_nodes(&id, reveal)
    }

    fn validate_current_step(&mut self, reveal: bool) -> bool {
        let validations = {
            let mut out = Vec::<(String, Result<(), String>)>::new();
            visit_nodes(self.flow.current_step().nodes.as_slice(), &mut |node| {
                out.push((node.id().to_string(), node.validate()));
            });
            out
        };

        let mut valid = true;
        for (id, result) in validations {
            if !self.apply_validation_result(&id, Some(result), reveal) {
                valid = false;
            }
        }
        valid
    }

    fn focus_first_invalid_on_current_step(&mut self) {
        let mut first_invalid: Option<String> = None;
        visit_nodes(self.flow.current_step().nodes.as_slice(), &mut |node| {
            if first_invalid.is_none() && self.validation.visible_error(node.id()).is_some() {
                first_invalid = Some(node.id().to_string());
            }
        });
        if let Some(id) = first_invalid {
            self.focus.set_focus_by_id(&id);
        }
    }

    fn validate_in_active_nodes(&mut self, id: &str, reveal: bool) -> bool {
        let mut validation_result: Option<Result<(), String>> = None;
        visit_nodes(self.active_nodes(), &mut |node| {
            if validation_result.is_none() && node.id() == id {
                validation_result = Some(node.validate());
            }
        });
        self.apply_validation_result(id, validation_result, reveal)
    }

    fn apply_validation_result(
        &mut self,
        id: &str,
        validation_result: Option<Result<(), String>>,
        reveal: bool,
    ) -> bool {
        match validation_result {
            Some(Ok(())) | None => {
                self.validation.clear_error(id);
                self.pending_scheduler.push(SchedulerCommand::Cancel {
                    key: inline_error_key(id),
                });
                true
            }
            Some(Err(error)) => {
                let visibility = if reveal {
                    ErrorVisibility::Inline
                } else {
                    ErrorVisibility::Hidden
                };
                self.validation.set_error(id.to_string(), error, visibility);
                if reveal {
                    self.pending_scheduler.push(SchedulerCommand::Debounce {
                        key: inline_error_key(id),
                        delay: ERROR_INLINE_TTL,
                        event: AppEvent::Widget(WidgetEvent::ClearInlineError {
                            id: id.to_string(),
                        }),
                    });
                }
                false
            }
        }
    }

    fn prune_validation_for_active_nodes(&mut self) {
        let mut ids = Vec::new();
        visit_nodes(self.active_nodes(), &mut |node| {
            ids.push(node.id().to_string())
        });
        self.validation.clear_for_ids(&ids);
    }

    pub fn take_pending_scheduler_commands(&mut self) -> Vec<SchedulerCommand> {
        self.pending_scheduler.drain(..).collect()
    }
}

fn inline_error_key(id: &str) -> String {
    format!("validation:inline:{id}")
}
