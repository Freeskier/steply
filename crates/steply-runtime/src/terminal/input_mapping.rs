use super::{
    KeyCode, KeyEvent, KeyModifiers, PointerButton, PointerEvent, PointerKind, PointerSemantic,
};
use crossterm::event::{
    KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent, KeyEventKind,
    KeyModifiers as CrosstermKeyModifiers, MouseButton as CrosstermMouseButton, MouseEvent,
    MouseEventKind,
};

pub(super) fn map_key_event(key: CrosstermKeyEvent) -> Option<KeyEvent> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    let modifiers = map_key_modifiers(key.modifiers);
    Some(KeyEvent {
        code: map_key_code(key.code, key.modifiers),
        modifiers,
    })
}

pub(super) fn map_pointer_event(mouse: MouseEvent) -> Option<PointerEvent> {
    let kind = match mouse.kind {
        MouseEventKind::Moved => PointerKind::Move,
        MouseEventKind::Down(button) => PointerKind::Down(map_pointer_button(button)?),
        MouseEventKind::Up(button) => PointerKind::Up(map_pointer_button(button)?),
        MouseEventKind::Drag(button) => PointerKind::Drag(map_pointer_button(button)?),
        MouseEventKind::ScrollUp
        | MouseEventKind::ScrollDown
        | MouseEventKind::ScrollLeft
        | MouseEventKind::ScrollRight => return None,
    };

    Some(PointerEvent {
        kind,
        col: mouse.column,
        row: mouse.row,
        modifiers: map_key_modifiers(mouse.modifiers),
        semantic: PointerSemantic::None,
    })
}

fn map_pointer_button(button: CrosstermMouseButton) -> Option<PointerButton> {
    match button {
        CrosstermMouseButton::Left => Some(PointerButton::Left),
        CrosstermMouseButton::Right => Some(PointerButton::Right),
        CrosstermMouseButton::Middle => Some(PointerButton::Middle),
    }
}

fn map_key_code(code: CrosstermKeyCode, modifiers: CrosstermKeyModifiers) -> KeyCode {
    match code {
        // Some terminals emit Ctrl+H as backspace control character.
        // Map it back to Ctrl+h so key bindings stay consistent.
        CrosstermKeyCode::Char('\u{8}')
            if modifiers.contains(CrosstermKeyModifiers::CONTROL)
                && !modifiers.contains(CrosstermKeyModifiers::ALT) =>
        {
            KeyCode::Char('h')
        }
        CrosstermKeyCode::Char('\u{8}' | '\u{7f}') => KeyCode::Backspace,
        CrosstermKeyCode::Char(ch) => KeyCode::Char(ch),
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => KeyCode::BackTab,
        CrosstermKeyCode::Esc => KeyCode::Esc,
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::PageUp => KeyCode::PageUp,
        CrosstermKeyCode::PageDown => KeyCode::PageDown,
        _ => KeyCode::Unknown,
    }
}

fn map_key_modifiers(modifiers: CrosstermKeyModifiers) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    if modifiers.contains(CrosstermKeyModifiers::SHIFT) {
        out = out.union(KeyModifiers::SHIFT);
    }
    if modifiers.contains(CrosstermKeyModifiers::CONTROL) {
        out = out.union(KeyModifiers::CONTROL);
    }
    if modifiers.contains(CrosstermKeyModifiers::ALT) {
        out = out.union(KeyModifiers::ALT);
    }
    out
}
