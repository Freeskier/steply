use super::AppState;
use crate::core::{NodeId, value::Value};
use crate::runtime::event::{AppEvent, SystemEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::validation::{ErrorVisibility, StepContext, StepIssue};
use crate::widgets::node::Node;
use crate::widgets::node::{NodeWalkScope, walk_nodes};
use crate::widgets::traits::ValidationMode;
use std::collections::HashMap;
use std::time::Duration;

const ERROR_INLINE_TTL: Duration = Duration::from_secs(2);

impl AppState {
    pub(super) fn validate_focused_live(&mut self) -> bool {
        self.validate_focused(ValidationMode::Live)
    }

    pub(super) fn validate_focused_submit(&mut self) -> bool {
        self.validate_focused(ValidationMode::Submit)
    }

    fn validate_focused(&mut self, mode: ValidationMode) -> bool {
        let Some(id) = self.ui.focus.current_id().map(|id| id.to_string()) else {
            return true;
        };
        self.validate_in_active_nodes(&id, mode)
    }

    pub(super) fn validate_current_step(&mut self, mode: ValidationMode) -> bool {
        self.runtime.validation.clear_step_errors();
        self.runtime.validation.clear_step_warnings();

        let validations = {
            let mut out = Vec::<(String, bool, Result<(), String>)>::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Persistent,
                &mut |node| {
                    out.push((
                        node.id().to_string(),
                        matches!(node, Node::Input(_)),
                        node.validate(mode),
                    ));
                },
            );
            out
        };

        let mut valid = true;
        let mut component_step_errors = Vec::<String>::new();
        for (id, is_input, result) in validations {
            let non_input_error = if mode == ValidationMode::Submit && !is_input {
                result.as_ref().err().cloned()
            } else {
                None
            };
            if !self.apply_validation_result(&id, Some(result), mode) {
                valid = false;
                if let Some(error) = non_input_error {
                    component_step_errors.push(error);
                }
            }
        }

        let (validator_errors, step_warnings) = self.collect_step_validator_issues();
        if !validator_errors.is_empty() {
            valid = false;
        }
        let mut step_errors = component_step_errors;
        step_errors.extend(validator_errors);

        self.runtime.validation.set_step_errors(step_errors);
        self.runtime.validation.set_step_warnings(step_warnings);

        valid
    }

    fn validate_in_active_nodes(&mut self, id: &str, mode: ValidationMode) -> bool {
        let mut result: Option<Result<(), String>> = None;
        walk_nodes(self.active_nodes(), NodeWalkScope::Visible, &mut |node| {
            if result.is_none() && node.id() == id {
                result = Some(node.validate(mode));
            }
        });
        self.apply_validation_result(id, result, mode)
    }

    fn apply_validation_result(
        &mut self,
        id: &str,
        result: Option<Result<(), String>>,
        mode: ValidationMode,
    ) -> bool {
        match result {
            Some(Ok(())) | None => {
                self.runtime.validation.clear_error(id);
                self.runtime
                    .pending_scheduler
                    .push(SchedulerCommand::Cancel {
                        key: inline_error_key(id),
                    });
                true
            }
            Some(Err(error)) => {
                let visibility = if mode == ValidationMode::Submit {
                    ErrorVisibility::Inline
                } else {
                    ErrorVisibility::Hidden
                };
                self.runtime.validation.set_error(id, error, visibility);
                if mode == ValidationMode::Submit {
                    self.runtime
                        .pending_scheduler
                        .push(SchedulerCommand::Debounce {
                            key: inline_error_key(id),
                            delay: ERROR_INLINE_TTL,
                            event: AppEvent::System(SystemEvent::ClearInlineError {
                                id: id.into(),
                            }),
                        });
                }
                false
            }
        }
    }

    pub(super) fn prune_validation_for_active_nodes(&mut self) {
        let mut ids = Vec::<NodeId>::new();
        walk_nodes(self.active_nodes(), NodeWalkScope::Visible, &mut |node| {
            ids.push(node.id().into())
        });
        self.runtime.validation.clear_for_ids(&ids);
        self.runtime.validation.clear_step_errors();
        self.runtime.validation.clear_step_warnings();
        self.runtime.validation.reset_warnings_acknowledged();
    }

    pub fn take_pending_scheduler_commands(&mut self) -> Vec<SchedulerCommand> {
        self.runtime.pending_scheduler.drain(..).collect()
    }

    fn collect_step_validator_issues(&self) -> (Vec<String>, Vec<String>) {
        let issues: Vec<StepIssue> = {
            let step = self.flow.current_step();
            if step.validators.is_empty() {
                return (Vec::new(), Vec::new());
            }
            let values = collect_node_values(step.nodes.as_slice());
            let ctx = StepContext::new(&step.id, &values);
            step.validators
                .iter()
                .filter_map(|validator| validator(&ctx))
                .collect()
        };

        let mut step_errors = Vec::new();
        let mut step_warnings = Vec::new();

        for issue in issues {
            match &issue {
                StepIssue::Error(msg) => {
                    step_errors.push(msg.clone());
                }
                StepIssue::Warning(msg) => {
                    step_warnings.push(msg.clone());
                }
            }
        }

        (step_errors, step_warnings)
    }
}

fn inline_error_key(id: &str) -> String {
    format!("validation:inline:{id}")
}

fn collect_node_values(nodes: &[Node]) -> HashMap<NodeId, Value> {
    let mut values = HashMap::<NodeId, Value>::new();
    walk_nodes(nodes, NodeWalkScope::Persistent, &mut |node| {
        if let Some(value) = node.value() {
            values.insert(node.id().into(), value);
        }
    });
    values
}
