use super::{AppState, bound_immediate_text_input, bound_on_submit_text_input, char_key};
use crate::core::value::Value;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::Node;

#[test]
fn immediate_binding_commits_live_while_focused() {
    let step = Step::builder("step_1", "Step")
        .node(bound_immediate_text_input("name", "Name", "profile::name"))
        .build();
    let mut state = AppState::new(Flow::new(vec![step])).expect("app state");

    state.dispatch_key_to_focused(char_key('A'));

    assert_eq!(
        state.store_value("profile::name"),
        Some(&Value::Text("A".to_string()))
    );
}

#[test]
fn on_submit_binding_preserves_draft_across_focus_changes_until_submit() {
    let step = Step::builder("step_1", "Step")
        .node(bound_on_submit_text_input("name", "Name", "draft_name"))
        .node(Node::Input(Box::new(TextInput::new("notes", "Notes"))))
        .build();
    let mut state = AppState::new(Flow::new(vec![step])).expect("app state");

    state.dispatch_key_to_focused(char_key('A'));
    assert_eq!(
        state.store_value("draft_name"),
        Some(&Value::Text(String::new()))
    );

    state.handle_tab_forward();
    state.dispatch_key_to_focused(char_key('B'));
    assert_eq!(
        state.store_value("draft_name"),
        Some(&Value::Text(String::new()))
    );

    state.handle_system_event(crate::runtime::event::SystemEvent::RequestSubmit);
    assert_eq!(
        state.store_value("draft_name"),
        Some(&Value::Text("A".to_string()))
    );
}
