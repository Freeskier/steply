use super::ArrayInput;
use crate::config::load_from_yaml_str;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::render_view::RenderView;
use crate::ui::renderer::{Renderer, RendererConfig};
use crate::widgets::traits::Interactive;

#[test]
fn cursor_stays_after_inserted_char_in_first_item() {
    let mut input = ArrayInput::new("tags", "Tags").with_items(vec!["rust".into(), "tui".into()]);

    input.on_key(KeyEvent {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
    });
    input.on_key(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
    });

    let cursor = input.cursor_pos().expect("cursor");
    assert_eq!(cursor.col, 6);
}

#[test]
fn rendered_frame_cursor_matches_array_input_offset() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: array_input
        id: tags
        label: Tags
        items: [rust, tui]
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let mut state = loaded.into_app_state().expect("app state");
    assert_eq!(state.focused_id(), Some("tags"));

    state.dispatch_key_to_focused(KeyEvent {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
    });
    state.dispatch_key_to_focused(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
    });

    let view = RenderView::from_state(&state);
    let mut renderer = Renderer::new(RendererConfig {
        chrome_enabled: false,
    });
    let frame = renderer.render(
        &view,
        crate::terminal::TerminalSize {
            width: 80,
            height: 20,
        },
    );

    assert_eq!(frame.cursor.expect("frame cursor").col, 12);
}

#[test]
fn bound_array_input_keeps_cursor_after_store_sync() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: array_input
        id: tags
        label: Tags
        items: [rust, tui]
        value: profile.tags
"#;

    let loaded = load_from_yaml_str(yaml).expect("load config");
    let mut state = loaded.into_app_state().expect("app state");
    assert_eq!(state.focused_id(), Some("tags"));

    state.dispatch_key_to_focused(KeyEvent {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
    });
    state.dispatch_key_to_focused(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
    });

    let view = RenderView::from_state(&state);
    let mut renderer = Renderer::new(RendererConfig {
        chrome_enabled: false,
    });
    let frame = renderer.render(
        &view,
        crate::terminal::TerminalSize {
            width: 80,
            height: 20,
        },
    );

    assert_eq!(frame.cursor.expect("frame cursor").col, 12);
}
