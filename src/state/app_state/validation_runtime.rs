use super::AppState;
use crate::core::{NodeId, value::Value};
use crate::runtime::event::{AppEvent, WidgetEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::validation::{
    ErrorVisibility, ValidationContext, ValidationIssue, ValidationTarget,
};
use crate::widgets::node::Node;
use crate::widgets::node::{NodeWalkScope, walk_nodes};
use std::collections::HashMap;
use std::time::Duration;

const ERROR_INLINE_TTL: Duration = Duration::from_secs(2);

impl AppState {
    pub(super) fn validate_focused(&mut self, reveal: bool) -> bool {
        let Some(id) = self.ui.focus.current_id().map(|id| id.to_string()) else {
            return true;
        };
        self.validate_in_active_nodes(&id, reveal)
    }

    pub(super) fn validate_current_step(&mut self, reveal: bool) -> bool {
        self.runtime.validation.clear_step_errors();

        let validations = {
            let mut out = Vec::<(String, Result<(), String>)>::new();
            walk_nodes(
                self.flow.current_step().nodes.as_slice(),
                NodeWalkScope::Persistent,
                &mut |node| {
                    let result = if reveal {
                        node.validate_submit()
                    } else {
                        node.validate_live()
                    };
                    out.push((node.id().to_string(), result));
                },
            );
            out
        };

        let mut valid = true;
        for (id, result) in validations {
            if !self.apply_validation_result(&id, Some(result), reveal) {
                valid = false;
            }
        }

        if !self.apply_step_validators(reveal) {
            valid = false;
        }

        valid
    }

    fn validate_in_active_nodes(&mut self, id: &str, reveal: bool) -> bool {
        let mut validation_result: Option<Result<(), String>> = None;
        walk_nodes(self.active_nodes(), NodeWalkScope::Visible, &mut |node| {
            if validation_result.is_none() && node.id() == id {
                let result = if reveal {
                    node.validate_submit()
                } else {
                    node.validate_live()
                };
                validation_result = Some(result);
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
                self.runtime.validation.clear_error(id);
                self.runtime
                    .pending_scheduler
                    .push(SchedulerCommand::Cancel {
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
                self.runtime
                    .validation
                    .set_error(id.to_string(), error, visibility);
                if reveal {
                    self.runtime
                        .pending_scheduler
                        .push(SchedulerCommand::Debounce {
                            key: inline_error_key(id),
                            delay: ERROR_INLINE_TTL,
                            event: AppEvent::Widget(WidgetEvent::ClearInlineError {
                                id: id.into(),
                            }),
                        });
                }
                false
            }
        }
    }

    pub(super) fn prune_validation_for_active_nodes(&mut self) {
        let mut ids = Vec::new();
        walk_nodes(self.active_nodes(), NodeWalkScope::Visible, &mut |node| {
            ids.push(node.id().to_string())
        });
        self.runtime.validation.clear_for_ids(&ids);
        self.runtime.validation.clear_step_errors();
    }

    pub fn take_pending_scheduler_commands(&mut self) -> Vec<SchedulerCommand> {
        self.runtime.pending_scheduler.drain(..).collect()
    }

    fn apply_step_validators(&mut self, reveal: bool) -> bool {
        let issues = {
            let step = self.flow.current_step();
            if step.validators.is_empty() {
                Vec::new()
            } else {
                let ctx = ValidationContext::new(
                    step.id.clone(),
                    collect_node_values(step.nodes.as_slice()),
                );
                step.validators
                    .iter()
                    .flat_map(|validator| validator(&ctx))
                    .collect::<Vec<ValidationIssue>>()
            }
        };

        let mut step_errors = Vec::new();
        let mut valid = true;
        for issue in issues {
            match issue.target {
                ValidationTarget::Node(id) => {
                    if !self.apply_validation_result(id.as_str(), Some(Err(issue.message)), reveal)
                    {
                        valid = false;
                    }
                }
                ValidationTarget::Step => {
                    valid = false;
                    if reveal {
                        step_errors.push(issue.message);
                    }
                }
            }
        }

        if reveal {
            self.runtime.validation.set_step_errors(step_errors);
        }

        valid
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
