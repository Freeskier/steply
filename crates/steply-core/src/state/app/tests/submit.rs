use super::{AppState, bound_on_submit_text_input, char_key};
use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::task::{TaskRequest, TaskSpec, TaskTrigger};

#[test]
fn submit_before_tasks_receive_submitted_on_submit_values() {
    let step = Step::builder("step_1", "Step")
        .node(bound_on_submit_text_input("name", "Name", "draft_name"))
        .build();
    let task = TaskSpec::exec("inspect", "cat", Vec::new())
        .with_reads(crate::widgets::shared::binding::ReadBinding::Selector(
            ValueTarget::node("draft_name"),
        ))
        .with_trigger(TaskTrigger::SubmitBefore {
            step_id: "step_1".to_string(),
        });
    let mut state = AppState::with_tasks(Flow::new(vec![step]), vec![task]).expect("app state");

    state.dispatch_key_to_focused(char_key('A'));
    state.handle_system_event(SystemEvent::RequestSubmit);

    let invocations = state.take_pending_task_invocations();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].stdin_json, "\"A\"");
}

#[test]
fn validate_and_task_request_commits_on_submit_values_before_starting_task() {
    let step = Step::builder("step_1", "Step")
        .node(bound_on_submit_text_input("name", "Name", "draft_name"))
        .build();
    let task = TaskSpec::exec("inspect", "cat", Vec::new()).with_reads(
        crate::widgets::shared::binding::ReadBinding::Selector(ValueTarget::node("draft_name")),
    );
    let mut state = AppState::with_tasks(Flow::new(vec![step]), vec![task]).expect("app state");

    state.dispatch_key_to_focused(char_key('A'));
    state.handle_action(WidgetAction::ValidateCurrentStepSubmitAndTaskRequest {
        request: TaskRequest::new("inspect"),
    });

    let invocations = state.take_pending_task_invocations();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].stdin_json, "\"A\"");
    assert_eq!(
        state.store_value("draft_name"),
        Some(&Value::Text("A".to_string()))
    );
}

#[test]
fn step_exit_and_submit_after_tasks_see_submitted_values() {
    let first = Step::builder("step_1", "Step")
        .node(bound_on_submit_text_input("name", "Name", "draft_name"))
        .build();
    let second = Step::builder("step_2", "Done").build();
    let exit_task = TaskSpec::exec("exit_inspect", "cat", Vec::new())
        .with_reads(crate::widgets::shared::binding::ReadBinding::Selector(
            ValueTarget::node("draft_name"),
        ))
        .with_trigger(TaskTrigger::StepExit {
            step_id: "step_1".to_string(),
        });
    let after_task = TaskSpec::exec("after_inspect", "cat", Vec::new())
        .with_reads(crate::widgets::shared::binding::ReadBinding::Selector(
            ValueTarget::node("draft_name"),
        ))
        .with_trigger(TaskTrigger::SubmitAfter {
            step_id: "step_1".to_string(),
        });
    let mut state =
        AppState::with_tasks(Flow::new(vec![first, second]), vec![exit_task, after_task])
            .expect("app state");

    state.dispatch_key_to_focused(char_key('A'));
    state.handle_system_event(SystemEvent::RequestSubmit);

    assert_eq!(state.current_step_id(), "step_2");

    let invocations = state.take_pending_task_invocations();
    assert_eq!(invocations.len(), 2);
    for invocation in invocations {
        assert_eq!(invocation.stdin_json, "\"A\"");
    }
}
