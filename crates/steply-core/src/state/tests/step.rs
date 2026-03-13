use super::super::step::{Step, StepCondition};
use crate::core::value::Value;
use crate::state::change::StoreCommitPolicy;
use crate::state::store::ValueStore;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::Node;
use crate::widgets::shared::binding::{
    ReadBinding, StoreBinding, WriteBinding, WriteExpr, bind_node,
};

#[test]
fn derived_writers_are_grouped_by_dependency_stage() {
    let step = Step::builder("step_1", "Step")
        .node(derived_copy_text_input(
            "middle_writer",
            "Middle",
            "source",
            "middle",
        ))
        .node(derived_copy_text_input(
            "sibling_writer",
            "Sibling",
            "source",
            "sibling",
        ))
        .node(derived_copy_text_input(
            "final_writer",
            "Final",
            "middle",
            "final",
        ))
        .build();

    let stages = &step.binding_plan.derived_writer_stages;
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].len(), 2);
    assert_eq!(stages[1].len(), 1);

    let first_stage_ids = stages[0]
        .iter()
        .map(|writer| writer.node_id.as_str())
        .collect::<Vec<_>>();
    assert!(first_stage_ids.contains(&"middle_writer"));
    assert!(first_stage_ids.contains(&"sibling_writer"));
    assert_eq!(stages[1][0].node_id, "final_writer");
}

#[test]
fn equals_condition_matches_store_value() {
    let mut store = ValueStore::new();
    let _ = store.set_target(
        &crate::core::store_refs::parse_store_selector("demo.enabled").expect("selector"),
        Value::Bool(true),
    );

    let condition = StepCondition::Equals {
        field: "demo.enabled".to_string(),
        value: Value::Bool(true),
    };

    assert!(condition.evaluate(&store));
}

#[test]
fn nested_all_and_not_conditions_match_expected_values() {
    let mut store = ValueStore::new();
    let _ = store.set_target(
        &crate::core::store_refs::parse_store_selector("demo.enabled").expect("selector"),
        Value::Bool(true),
    );
    let _ = store.set_target(
        &crate::core::store_refs::parse_store_selector("demo.blocked").expect("selector"),
        Value::Bool(false),
    );

    let condition = StepCondition::All(vec![
        StepCondition::Truthy {
            field: "demo.enabled".to_string(),
        },
        StepCondition::Not(Box::new(StepCondition::Truthy {
            field: "demo.blocked".to_string(),
        })),
    ]);

    assert!(condition.evaluate(&store));
}

fn derived_copy_text_input(
    id: &str,
    label: &str,
    read_selector: &str,
    write_selector: &str,
) -> Node {
    bind_node(
        Node::Input(Box::new(TextInput::new(id, label))),
        StoreBinding {
            value: None,
            options: None,
            reads: Some(ReadBinding::Selector(
                crate::core::value_path::ValueTarget::node(read_selector),
            )),
            writes: vec![WriteBinding {
                target: crate::core::value_path::ValueTarget::node(write_selector),
                expr: WriteExpr::ScopeRef("value".to_string()),
            }],
            commit_policy: StoreCommitPolicy::Immediate,
        },
    )
}
