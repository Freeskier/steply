use super::{AppState, bound_immediate_text_input, char_key};
use crate::config::load_from_yaml_str;
use crate::core::value::Value;
use crate::runtime::event::SystemEvent;
use crate::state::change::StoreWriteOrigin;
use crate::state::flow::Flow;
use crate::state::step::{Step, StepCondition};
use crate::task::{ConcurrencyPolicy, TaskCompletion};
use crate::terminal::TerminalSize;
use crate::ui::render_view::RenderView;
use crate::ui::renderer::{Renderer, RendererConfig};
use crate::widgets::node::find_node;
use crate::widgets::shared::condition::wrap_node_when;
use crate::widgets::traits::FocusMode;

#[test]
fn truthy_when_hides_disabled_step() {
    let yaml = r#"
version: 1
steps:
  - id: setup
    title: Setup
    widgets:
      - type: checkbox
        id: enabled
        label: Enabled
        default: false
        value: demo.enabled
  - id: gated
    title: Gated
    when:
      ref: demo.enabled
    widgets: []
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let state = loaded.into_app_state().expect("app state");

    assert_eq!(state.visible_step_indices(), vec![0]);
}

#[test]
fn widget_when_hides_and_reveals_focusable_node() {
    let step = Step::builder("setup", "Setup")
        .node(bound_immediate_text_input(
            "advanced",
            "Advanced",
            "demo.advanced",
        ))
        .node(wrap_node_when(
            bound_immediate_text_input("details", "Details", "demo.details"),
            StepCondition::Truthy {
                field: "demo.advanced".to_string(),
            },
        ))
        .build();
    let mut state = AppState::new(Flow::new(vec![step])).expect("app state");

    let details = find_node(&state.steps()[0].nodes, "details").expect("details node");
    assert_eq!(details.focus_mode(), FocusMode::None);
    assert_eq!(state.focused_id(), Some("advanced"));

    state.dispatch_key_to_focused(char_key('A'));

    let details = find_node(&state.steps()[0].nodes, "details").expect("details node");
    assert_eq!(details.focus_mode(), FocusMode::Leaf);
}

#[test]
fn widget_when_uses_empty_and_not_empty_from_yaml_for_components_and_outputs() {
    let yaml = r#"
version: 1
tasks:
  - id: remaining
    kind: exec
    program: cat
    writes: demo.remaining_files
steps:
  - id: demo
    title: Demo
    widgets:
      - type: repeater
        id: groups
        label: Groups
        iterate: demo.remaining_files
        entry_mode: full
        when:
          ref: demo.remaining_files
          is: not_empty
        widgets:
          - type: text_input
            id: name
            label: Name
      - type: text_output
        id: done
        text: "Done"
        when:
          ref: demo.remaining_files
          is: empty
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let mut state = loaded.into_app_state().expect("app state");
    state.apply_system_value_change(
        crate::core::store_refs::parse_store_selector("demo.remaining_files").expect("selector"),
        Value::List(vec![Value::Text("Cargo.toml".into())]),
        StoreWriteOrigin::System,
    );

    let view = RenderView::from_state(&state);
    let mut renderer = Renderer::new(RendererConfig {
        chrome_enabled: false,
    });
    let frame = renderer.render(
        &view,
        TerminalSize {
            width: 80,
            height: 20,
        },
    );
    let rendered = frame
        .lines
        .iter()
        .flat_map(|line| line.iter().map(|span| span.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!("rendered output:\n{rendered}");
    assert!(rendered.contains("Groups"), "rendered output:\n{rendered}");
    assert!(!rendered.contains("Done"), "rendered output:\n{rendered}");
}

#[test]
fn step_enter_task_completion_refreshes_widget_conditions() {
    let yaml = r#"
version: 1
tasks:
  - id: seed_remaining
    kind: exec
    program: cat
    triggers:
      - type: step_enter
        step_id: gated
    writes: demo.remaining_files
steps:
  - id: setup
    title: Setup
    widgets:
      - type: text_input
        id: proceed
        label: Proceed
        default: ready
        value: demo.proceed
  - id: gated
    title: Gated
    widgets:
      - type: repeater
        id: groups
        label: Groups
        iterate: demo.remaining_files
        entry_mode: full
        when:
          ref: demo.remaining_files
          is: not_empty
        widgets:
          - type: text_input
            id: name
            label: Name
      - type: text_output
        id: done
        text: "Done"
        when:
          ref: demo.remaining_files
          is: empty
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let mut state = loaded.into_app_state().expect("app state");

    state.handle_system_event(SystemEvent::RequestSubmit);

    let invocations = state.take_pending_task_invocations();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].spec.id.as_str(), "seed_remaining");
    assert_eq!(invocations[0].spec.writes.len(), 1);

    let accepted = crate::task::engine::complete_task_run(
        &mut state,
        TaskCompletion {
            task_id: "seed_remaining".into(),
            run_id: invocations[0].run_id,
            concurrency_policy: ConcurrencyPolicy::Parallel,
            result: Value::List(vec![
                Value::Text("alpha".into()),
                Value::Text("beta".into()),
            ]),
            error: None,
            cancelled: false,
        },
    );

    assert!(accepted);
    assert_eq!(state.current_step_id(), "gated");
    assert_eq!(state.current_step_errors(), &[] as &[String]);
    assert_eq!(
        state.store_value("demo.remaining_files"),
        Some(&Value::List(vec![
            Value::Text("alpha".into()),
            Value::Text("beta".into())
        ]))
    );

    let view = RenderView::from_state(&state);
    let mut renderer = Renderer::new(RendererConfig {
        chrome_enabled: false,
    });
    let frame = renderer.render(
        &view,
        TerminalSize {
            width: 80,
            height: 20,
        },
    );
    let rendered = frame
        .lines
        .iter()
        .flat_map(|line| line.iter().map(|span| span.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Groups"), "rendered output:\n{rendered}");
    assert!(!rendered.contains("Done"), "rendered output:\n{rendered}");
}
