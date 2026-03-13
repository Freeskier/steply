use std::path::PathBuf;

use super::*;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};

fn space_key() -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(' '),
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn list_space_does_not_select_directory_in_multi_mode() {
    let mut browser = FileBrowserComponent::new("files", "Files")
        .with_selection_mode(SelectionMode::Multi)
        .with_browser_mode(BrowserMode::List);
    browser.overlay_open = true;
    browser.list_overlay_items = vec![ActiveOverlayItem::Entry {
        path: PathBuf::from("src"),
        is_dir: true,
    }];

    let result = browser.handle_browser_key(space_key());

    assert!(result.handled);
    assert!(browser.selected_paths.is_empty());
}

#[test]
fn list_space_still_selects_file_in_multi_mode() {
    let mut browser = FileBrowserComponent::new("files", "Files")
        .with_selection_mode(SelectionMode::Multi)
        .with_browser_mode(BrowserMode::List);
    browser.overlay_open = true;
    browser.list_overlay_items = vec![ActiveOverlayItem::Entry {
        path: PathBuf::from("src/main.rs"),
        is_dir: false,
    }];

    let result = browser.handle_browser_key(space_key());

    assert!(result.handled);
    assert_eq!(browser.selected_paths, vec![PathBuf::from("src/main.rs")]);
}

#[test]
fn multi_select_preserves_active_directory_query_after_file_toggle() {
    let mut browser = FileBrowserComponent::new("files", "Files")
        .with_selection_mode(SelectionMode::Multi)
        .with_browser_mode(BrowserMode::List);
    browser.overlay_open = true;
    browser
        .text
        .set_value(crate::core::value::Value::Text("src/".into()));
    browser.list_overlay_items = vec![ActiveOverlayItem::Entry {
        path: PathBuf::from("src/main.rs"),
        is_dir: false,
    }];

    let result = browser.handle_browser_key(space_key());

    assert!(result.handled);
    assert_eq!(browser.query_input(), "src/");
    assert_eq!(browser.current_input(), "src/main.rs, src/");
}
