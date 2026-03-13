use super::{AppState, bound_immediate_text_input, char_key, derived_copy_text_input};
use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::runtime::event::SystemEvent;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::task::{ConcurrencyPolicy, TaskCompletion, TaskSpec, TaskTrigger};
use crate::widgets::shared::binding::{ReadBinding, WriteBinding, WriteExpr};

#[test]
fn task_store_writes_run_through_same_runtime_pipeline_as_other_store_updates() {
    let step = Step::builder("step_1", "Step")
        .node(derived_copy_text_input(
            "middle_writer",
            "Middle",
            "source",
            "middle",
        ))
        .node(derived_copy_text_input(
            "final_writer",
            "Final",
            "middle",
            "final",
        ))
        .build();
    let task = TaskSpec::exec("fill_source", "cat", Vec::new()).with_writes(vec![WriteBinding {
        target: ValueTarget::node("source"),
        expr: WriteExpr::ScopeRef("result".to_string()),
    }]);
    let mut state = AppState::with_tasks(Flow::new(vec![step]), vec![task]).expect("app state");

    state.handle_system_event(SystemEvent::TaskCompleted {
        completion: TaskCompletion {
            task_id: "fill_source".into(),
            run_id: 1,
            concurrency_policy: ConcurrencyPolicy::Parallel,
            result: Value::Text("A".to_string()),
            error: None,
            cancelled: false,
        },
    });

    assert_eq!(
        state.store_value("source"),
        Some(&Value::Text("A".to_string()))
    );
    assert_eq!(
        state.store_value("middle"),
        Some(&Value::Text("A".to_string()))
    );
    assert_eq!(
        state.store_value("final"),
        Some(&Value::Text("A".to_string()))
    );
}

#[test]
fn store_changed_triggers_fire_for_overlapping_parent_selectors() {
    let step = Step::builder("step_1", "Step")
        .node(bound_immediate_text_input("name", "Name", "profile::name"))
        .build();
    let task = TaskSpec::exec("watch_profile", "cat", Vec::new())
        .with_reads(ReadBinding::Selector(ValueTarget::node("profile")))
        .with_trigger(TaskTrigger::StoreChanged {
            selector: ValueTarget::node("profile"),
            debounce_ms: 0,
        });
    let mut state = AppState::with_tasks(Flow::new(vec![step]), vec![task]).expect("app state");
    let _ = state.take_pending_task_invocations();

    state.dispatch_key_to_focused(char_key('A'));

    let invocations = state.take_pending_task_invocations();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].stdin_json, "{\"name\":\"A\"}");
}
