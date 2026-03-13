use super::{AppState, derived_copy_text_input};
use crate::core::value::Value;
use crate::state::flow::Flow;
use crate::state::step::Step;

#[test]
fn derived_bindings_propagate_across_dependency_stages() {
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
    let mut state = AppState::new(Flow::new(vec![step])).expect("app state");

    state.apply_value_change("source", Value::Text("A".to_string()));

    assert_eq!(
        state.store_value("middle"),
        Some(&Value::Text("A".to_string()))
    );
    assert_eq!(
        state.store_value("final"),
        Some(&Value::Text("A".to_string()))
    );
}
