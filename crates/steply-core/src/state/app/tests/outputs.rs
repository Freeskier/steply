use crate::config::load_from_yaml_str;
use crate::core::value::Value;
use crate::terminal::TerminalSize;
use crate::ui::render_view::RenderView;
use crate::ui::renderer::{Renderer, RendererConfig};
use crate::widgets::node::find_node;

#[test]
fn text_output_template_still_reads_store_through_binding_pipeline() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_input
        id: name
        label: Name
        default: Ada
        value: demo.name
      - type: text_output
        id: greeting
        text: "Hello {{demo.name}}"
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let state = loaded.into_app_state().expect("app state");
    let greeting = find_node(&state.steps()[0].nodes, "greeting")
        .and_then(|node| node.value())
        .expect("greeting value");

    assert_eq!(greeting, Value::Text("Hello Ada".to_string()));
}

#[test]
fn progress_output_supports_read_only_binding_reads() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: slider
        id: count
        label: Count
        min: 0
        max: 10
        default: 4
        value: demo.count
      - type: progress_output
        id: progress
        label: Progress
        min: 0
        max: 10
        reads: demo.count
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let state = loaded.into_app_state().expect("app state");
    let progress = find_node(&state.steps()[0].nodes, "progress")
        .and_then(|node| node.value())
        .expect("progress value");

    assert_eq!(progress, Value::Number(4.0));
}

#[test]
fn text_output_wraps_when_line_is_wider_than_viewport() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_output
        id: note
        text: "This is a long wrapped output line"
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let state = loaded.into_app_state().expect("app state");
    let view = RenderView::from_state(&state);
    let mut renderer = Renderer::new(RendererConfig {
        chrome_enabled: false,
    });
    let frame = renderer.render(
        &view,
        TerminalSize {
            width: 16,
            height: 20,
        },
    );
    let rendered = frame
        .lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    assert!(
        rendered.iter().any(|line| line.contains("wrapped")),
        "rendered lines: {rendered:#?}"
    );
    assert!(rendered.len() > 2, "rendered lines: {rendered:#?}");
}
