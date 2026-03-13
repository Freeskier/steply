use super::{Repeater, RepeaterEntryMode, resolved_iterate_state};
use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::state::step::StepCondition;
use crate::state::store::ValueStore;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::span::SpanLine;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::Node;
use crate::widgets::outputs::text::TextOutput;
use crate::widgets::shared::binding::{
    ReadBinding, StoreBinding, WriteBinding, WriteExpr, bind_node,
};
use crate::widgets::shared::condition::wrap_node_when;
use crate::widgets::traits::{Drawable, Interactive, RenderContext};

#[test]
fn iterate_binding_uses_list_items_and_private_scope() {
    let mut store = ValueStore::new();
    let target = ValueTarget::parse_selector("demo.items").expect("selector");
    store
        .set_target(
            &target,
            Value::List(vec![
                Value::Text("alpha".into()),
                Value::Text("beta".into()),
            ]),
        )
        .expect("set list");

    let mut repeater = Repeater::new("groups", "Groups")
        .with_iterate_binding(ReadBinding::Selector(target))
        .with_widget(Node::Input(Box::new(TextInput::new("name", "Name"))));

    assert!(repeater.sync_from_store(&store));
    assert_eq!(repeater.total_count(), 2);
    assert_eq!(repeater.current_item(), Some(&Value::Text("alpha".into())));
    assert_eq!(
        repeater.scoped_store().get("_count"),
        Some(&Value::Number(2.0))
    );
    assert_eq!(
        repeater.scoped_store().get("_item"),
        Some(&Value::Text("alpha".into()))
    );
}

#[test]
fn progressive_entry_mode_renders_only_active_widget() {
    let repeater = Repeater::new("groups", "Groups")
        .with_count(1)
        .with_entry_mode(RepeaterEntryMode::Progressive)
        .with_widget(Node::Input(Box::new(
            TextInput::new("name", "Name").with_default(Value::Text("alpha".into())),
        )))
        .with_widget(Node::Input(Box::new(
            TextInput::new("path", "Path").with_default(Value::Text("beta".into())),
        )));

    let rendered = rendered_text(repeater.draw(&RenderContext::empty(TerminalSize {
        width: 80,
        height: 20,
    })));

    assert!(rendered.contains("alpha"));
    assert!(!rendered.contains("beta"));
}

#[test]
fn full_entry_mode_renders_all_widgets_for_iteration() {
    let repeater = Repeater::new("groups", "Groups")
        .with_count(1)
        .with_entry_mode(RepeaterEntryMode::Full)
        .with_widget(Node::Input(Box::new(
            TextInput::new("name", "Name").with_default(Value::Text("alpha".into())),
        )))
        .with_widget(Node::Input(Box::new(
            TextInput::new("path", "Path").with_default(Value::Text("beta".into())),
        )));

    let rendered = rendered_text(repeater.draw(&RenderContext::empty(TerminalSize {
        width: 80,
        height: 20,
    })));

    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("beta"));
}

#[test]
fn resolved_iterate_state_supports_count_values() {
    let (items, count) = resolved_iterate_state(Some(&Value::Number(3.0)));

    assert!(items.is_empty());
    assert_eq!(count, 3);
}

#[test]
fn enter_commits_seeded_active_widget_value_into_row() {
    let mut repeater = Repeater::new("groups", "Groups")
        .with_iterate_binding(ReadBinding::Literal(Value::Number(1.0)))
        .with_widget(bind_node(
            Node::Input(Box::new(TextInput::new("name", "Name"))),
            StoreBinding {
                reads: Some(ReadBinding::Literal(Value::Text("Group 1".into()))),
                writes: vec![WriteBinding {
                    target: ValueTarget::node("name"),
                    expr: WriteExpr::ScopeRef("value".into()),
                }],
                ..StoreBinding::default()
            },
        ));

    let store = ValueStore::new();
    assert!(repeater.sync_from_store(&store));

    let result = repeater.on_key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    });

    assert!(result.handled);
    assert_eq!(
        repeater.value(),
        Some(Value::List(vec![Value::Object(
            [("name".into(), Value::Text("Group 1".into()))]
                .into_iter()
                .collect()
        )]))
    );
}

#[test]
fn hidden_child_in_full_mode_does_not_render_gap_or_take_focus() {
    let mut store = ValueStore::new();
    store
        .set("demo.visible", Value::Bool(false))
        .expect("set visibility");

    let mut repeater = Repeater::new("groups", "Groups")
        .with_iterate_binding(ReadBinding::Literal(Value::Number(1.0)))
        .with_entry_mode(RepeaterEntryMode::Full)
        .with_widget(bind_node(
            Node::Input(Box::new(TextInput::new("first", "First"))),
            StoreBinding {
                reads: Some(ReadBinding::Literal(Value::Text("alpha".into()))),
                writes: vec![WriteBinding {
                    target: ValueTarget::node("first"),
                    expr: WriteExpr::ScopeRef("value".into()),
                }],
                ..StoreBinding::default()
            },
        ))
        .with_widget(wrap_node_when(
            bind_node(
                Node::Input(Box::new(TextInput::new("hidden", "Hidden"))),
                StoreBinding {
                    reads: Some(ReadBinding::Literal(Value::Text("beta".into()))),
                    writes: vec![WriteBinding {
                        target: ValueTarget::node("hidden"),
                        expr: WriteExpr::ScopeRef("value".into()),
                    }],
                    ..StoreBinding::default()
                },
            ),
            StepCondition::Truthy {
                field: "demo.visible".into(),
            },
        ))
        .with_widget(bind_node(
            Node::Input(Box::new(TextInput::new("third", "Third"))),
            StoreBinding {
                reads: Some(ReadBinding::Literal(Value::Text("gamma".into()))),
                writes: vec![WriteBinding {
                    target: ValueTarget::node("third"),
                    expr: WriteExpr::ScopeRef("value".into()),
                }],
                ..StoreBinding::default()
            },
        ));

    assert!(repeater.sync_from_store(&store));

    let rendered = rendered_text(repeater.draw(&RenderContext::empty(TerminalSize {
        width: 80,
        height: 20,
    })));

    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("gamma"));
    assert!(!rendered.contains("beta"));
    assert!(!rendered.contains("\n\n"));

    let result = repeater.on_key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    });

    assert!(result.handled);
    assert_eq!(repeater.active_widget, 2);
}

#[test]
fn long_nowrap_output_after_prefixed_row_does_not_create_blank_line() {
    let mut repeater = Repeater::new("groups", "Groups")
        .with_iterate_binding(ReadBinding::Literal(Value::Number(1.0)))
        .with_entry_mode(RepeaterEntryMode::Full)
        .with_widget(bind_node(
            Node::Input(Box::new(TextInput::new("name", "Name"))),
            StoreBinding {
                reads: Some(ReadBinding::Literal(Value::Text("Group 0".into()))),
                writes: vec![WriteBinding {
                    target: ValueTarget::node("name"),
                    expr: WriteExpr::ScopeRef("value".into()),
                }],
                ..StoreBinding::default()
            },
        ))
        .with_widget(Node::Output(Box::new(TextOutput::new(
            "preview",
            "Current draft: {\"initial_file\":\"Cargo.toml\",\"name\":\"Group 0\",\"selected_files\":[\"Cargo.lock\",\"Cargo.toml\"]}",
        ))));

    let store = ValueStore::new();
    assert!(repeater.sync_from_store(&store));

    let output = repeater.draw(&RenderContext::empty(TerminalSize {
        width: 40,
        height: 20,
    }));
    let rendered = Layout::compose(output.lines.as_slice(), 40)
        .into_iter()
        .map(rendered_line)
        .collect::<Vec<_>>();

    assert!(
        !rendered.iter().any(|line| line.trim().is_empty()),
        "rendered lines: {rendered:#?}"
    );
    assert!(
        rendered.iter().any(|line| line.contains("Current draft:")),
        "rendered lines: {rendered:#?}"
    );
}

fn rendered_text(output: crate::widgets::traits::DrawOutput) -> String {
    output
        .lines
        .into_iter()
        .map(rendered_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn rendered_line(line: SpanLine) -> String {
    line.into_iter().map(|span| span.text).collect()
}
