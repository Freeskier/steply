use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::state::app::AppState;
use crate::task::engine::{complete_task_run, request_task_run};
use crate::widgets::node::{NodeWalkScope, find_node, walk_nodes_mut};
use crate::widgets::traits::{InteractionResult, ValidationMode};

pub(super) struct EffectDispatcher<'a> {
    state: &'a mut AppState,
}

impl<'a> EffectDispatcher<'a> {
    pub(super) fn new(state: &'a mut AppState) -> Self {
        Self { state }
    }

    pub(super) fn handle_action(&mut self, action: WidgetAction) -> InteractionResult {
        match action {
            WidgetAction::ValueChanged { change } => {
                self.state
                    .apply_value_change_target(change.target, change.value);
                self.state.clear_completion_session();
                self.state.clear_step_errors();
                InteractionResult::handled()
            }
            WidgetAction::OpenUrl { .. } => InteractionResult::consumed(),
            WidgetAction::InputDone => {
                if self.state.has_blocking_overlay() {
                    self.state.close_overlay();
                } else if self.state.pending_back_confirm.is_some() {
                    self.state.confirm_back();
                } else if self.state.ui.focus.is_last() {
                    self.state.handle_step_submit();
                } else {
                    self.state.focus_next();
                }
                InteractionResult::handled()
            }
            WidgetAction::ValidateFocusedSubmit => {
                self.state.validate_focused_submit();
                InteractionResult::handled()
            }
            WidgetAction::ValidateCurrentStepSubmit => {
                self.state.validate_current_step(ValidationMode::Submit);
                InteractionResult::handled()
            }
            WidgetAction::ValidateCurrentStepSubmitAndTaskRequest { request } => {
                if self.state.validate_current_step(ValidationMode::Submit) {
                    request_task_run(self.state, request);
                }
                InteractionResult::handled()
            }
            WidgetAction::RequestFocus { target } => {
                if !self.state.ui.active_node_index.has_visible(target.as_str())
                    && find_node(self.state.active_nodes(), target.as_str()).is_none()
                {
                    return InteractionResult::ignored();
                }
                self.state.clear_completion_session();
                self.state.ui.focus.set_focus_by_id(target.as_str());

                let focus_event = SystemEvent::RequestFocus {
                    target: Some(target),
                };
                let result = self.broadcast_system_event(&focus_event);
                self.process_broadcast_result(result);
                InteractionResult::handled()
            }
            WidgetAction::TaskRequested { request } => {
                request_task_run(self.state, request);
                InteractionResult::handled()
            }
        }
    }

    pub(super) fn handle_system_event(&mut self, event: SystemEvent) -> InteractionResult {
        match event {
            SystemEvent::ClearInlineError { id } => {
                self.state.runtime.validation.clear_error(id.as_str());
                InteractionResult::handled()
            }
            SystemEvent::OpenOverlay { overlay_id } => {
                if self.state.open_overlay_by_id(overlay_id.as_str()) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            SystemEvent::CloseOverlay => {
                self.state.close_overlay();
                InteractionResult::handled()
            }
            SystemEvent::OverlayLifecycle { .. } | SystemEvent::RequestFocus { .. } => {
                InteractionResult::ignored()
            }
            SystemEvent::TaskRequested { request } => {
                request_task_run(self.state, request);
                InteractionResult::handled()
            }
            SystemEvent::TaskStarted { .. }
            | SystemEvent::TaskStartRejected { .. }
            | SystemEvent::TaskLogLine { .. } => {
                let result = self.broadcast_system_event(&event);
                self.process_broadcast_result(result);
                InteractionResult::handled()
            }
            SystemEvent::TaskCompleted { ref completion } => {
                let accepted = complete_task_run(self.state, completion.clone());
                if accepted {
                    let result = self.broadcast_system_event(&event);
                    self.process_broadcast_result(result);
                }
                InteractionResult::handled()
            }
            SystemEvent::RequestSubmit => {
                if self.state.has_blocking_overlay() {
                    self.state.close_overlay();
                } else {
                    self.state.handle_step_submit();
                }
                InteractionResult::handled()
            }
        }
    }

    pub(super) fn broadcast_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let mut merged = InteractionResult::ignored();
        for step in self.state.flow.steps_mut() {
            walk_nodes_mut(
                step.nodes.as_mut_slice(),
                NodeWalkScope::Recursive,
                &mut |node| merged.merge(node.on_system_event(event)),
            );
        }
        merged
    }

    pub(super) fn process_broadcast_result(&mut self, result: InteractionResult) {
        for action in result.actions {
            let _ = self.handle_action(action);
        }
    }
}

impl AppState {
    fn effect_dispatcher(&mut self) -> EffectDispatcher<'_> {
        EffectDispatcher::new(self)
    }

    pub fn handle_action(&mut self, action: WidgetAction) -> InteractionResult {
        self.effect_dispatcher().handle_action(action)
    }

    pub fn handle_system_event(&mut self, event: SystemEvent) -> InteractionResult {
        self.effect_dispatcher().handle_system_event(event)
    }

    pub(in crate::state::app) fn broadcast_system_event(
        &mut self,
        event: &SystemEvent,
    ) -> InteractionResult {
        self.effect_dispatcher().broadcast_system_event(event)
    }

    pub(in crate::state::app) fn process_broadcast_result(&mut self, result: InteractionResult) {
        self.effect_dispatcher().process_broadcast_result(result)
    }
}
