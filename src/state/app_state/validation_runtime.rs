use super::AppState;
use crate::runtime::event::{AppEvent, WidgetEvent};
use crate::runtime::scheduler::SchedulerCommand;
use crate::state::validation::{
    ErrorVisibility, ValidationContext, ValidationIssue, ValidationTarget,
};
use crate::widgets::node::Node;
use crate::widgets::node::visit_nodes;
use std::collections::HashMap;
use std::time::Duration;

const ERROR_INLINE_TTL: Duration = Duration::from_secs(2);

impl AppState {
    pub(super) fn validate_focused(&mut self, reveal: bool) -> bool {
        let Some(id) = self.focus.current_id().map(|id| id.to_string()) else {
            return true;
        };
        self.validate_in_active_nodes(&id, reveal)
    }

    pub(super) fn validate_current_step(&mut self, reveal: bool) -> bool {
        self.validation.clear_step_errors();

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

        if !self.apply_step_validators(reveal) {
            valid = false;
        }

        valid
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

    pub(super) fn prune_validation_for_active_nodes(&mut self) {
        let mut ids = Vec::new();
        visit_nodes(self.active_nodes(), &mut |node| {
            ids.push(node.id().to_string())
        });
        self.validation.clear_for_ids(&ids);
        self.validation.clear_step_errors();
    }

    pub fn take_pending_scheduler_commands(&mut self) -> Vec<SchedulerCommand> {
        self.pending_scheduler.drain(..).collect()
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
                    if !self.apply_validation_result(&id, Some(Err(issue.message)), reveal) {
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
            self.validation.set_step_errors(step_errors);
        }

        valid
    }
}

fn inline_error_key(id: &str) -> String {
    format!("validation:inline:{id}")
}

fn collect_node_values(nodes: &[Node]) -> HashMap<String, crate::core::value::Value> {
    let mut values = HashMap::new();
    visit_nodes(nodes, &mut |node| {
        if let Some(value) = node.value() {
            values.insert(node.id().to_string(), value);
        }
    });
    values
}
