use crate::runtime::intent::Intent;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::traits::TextAction;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn key(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }

    pub fn ctrl(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::CONTROL)
    }

    pub fn alt(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::ALT)
    }

    pub fn from_event(event: KeyEvent) -> Self {
        Self {
            code: event.code,
            modifiers: event.modifiers,
        }
    }
}

#[derive(Default)]
pub struct KeyBindings {
    bindings: HashMap<KeyBinding, Intent>,
}

impl KeyBindings {
    pub fn new() -> Self {
        let mut manager = Self::default();
        manager.install_defaults();
        manager
    }

    pub fn bind(&mut self, key: KeyBinding, intent: Intent) {
        self.bindings.insert(key, intent);
    }

    pub fn unbind(&mut self, key: &KeyBinding) {
        self.bindings.remove(key);
    }

    pub fn resolve(&self, event: KeyEvent) -> Option<Intent> {
        if is_copy_selection_shortcut(event) {
            return Some(Intent::CopySelection);
        }
        if let Some(intent) = self.bindings.get(&KeyBinding::from_event(event)).cloned() {
            return Some(intent);
        }

        // Terminal compatibility:
        // some terminals report Ctrl+Shift+<letter> as uppercase char.
        // Fall back to Ctrl+<lowercase letter> bindings when possible.
        normalized_ctrl_char_event(event).and_then(|normalized| {
            self.bindings
                .get(&KeyBinding::from_event(normalized))
                .cloned()
        })
    }

    fn install_defaults(&mut self) {
        self.bind(KeyBinding::ctrl(KeyCode::Char('c')), Intent::Exit);
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('o')),
            Intent::OpenOverlayShortcut,
        );

        self.bind(KeyBinding::ctrl(KeyCode::Char('/')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('?')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('_')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('7')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('h')), Intent::ToggleHints);
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('1')),
            Intent::OpenOverlayAtIndex(0),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('2')),
            Intent::OpenOverlayAtIndex(1),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('3')),
            Intent::OpenOverlayAtIndex(2),
        );

        self.bind(
            KeyBinding::alt(KeyCode::Char('1')),
            Intent::OpenOverlayAtIndex(0),
        );
        self.bind(
            KeyBinding::alt(KeyCode::Char('2')),
            Intent::OpenOverlayAtIndex(1),
        );
        self.bind(
            KeyBinding::alt(KeyCode::Char('3')),
            Intent::OpenOverlayAtIndex(2),
        );
        self.bind(KeyBinding::key(KeyCode::Esc), Intent::Cancel);
        self.bind(KeyBinding::alt(KeyCode::Left), Intent::Back);
        self.bind(KeyBinding::key(KeyCode::Tab), Intent::CompleteNext);

        self.bind(
            KeyBinding::ctrl(KeyCode::Char(' ')),
            Intent::ToggleCompletion,
        );
        self.bind(
            KeyBinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            Intent::CompletePrev,
        );
        // Terminal compatibility: some environments report BackTab without SHIFT flag.
        self.bind(KeyBinding::key(KeyCode::BackTab), Intent::CompletePrev);
        self.bind(KeyBinding::alt(KeyCode::Down), Intent::Submit);
        self.bind(KeyBinding::alt(KeyCode::Up), Intent::Back);
        self.bind(
            KeyBinding::ctrl(KeyCode::Left),
            Intent::TextAction(TextAction::MoveWordLeft),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Right),
            Intent::TextAction(TextAction::MoveWordRight),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Backspace),
            Intent::TextAction(TextAction::DeleteWordLeft),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('w')),
            Intent::TextAction(TextAction::DeleteWordLeft),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Delete),
            Intent::TextAction(TextAction::DeleteWordRight),
        );
        self.bind(KeyBinding::key(KeyCode::PageUp), Intent::ScrollPageUp);
        self.bind(KeyBinding::key(KeyCode::PageDown), Intent::ScrollPageDown);
        let ctrl_shift = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT);
        self.bind(
            KeyBinding::new(KeyCode::Char('c'), ctrl_shift),
            Intent::CopySelection,
        );
        self.bind(
            KeyBinding::new(KeyCode::Char('C'), ctrl_shift),
            Intent::CopySelection,
        );
        self.bind(KeyBinding::alt(KeyCode::Char('c')), Intent::CopySelection);
    }
}

fn is_copy_selection_shortcut(event: KeyEvent) -> bool {
    if !event.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }
    match event.code {
        // Standard explicit binding: Ctrl+Shift+C.
        KeyCode::Char('c') => event.modifiers.contains(KeyModifiers::SHIFT),
        // Compatibility binding:
        // Some terminals fold Shift into uppercase char and omit SHIFT modifier.
        KeyCode::Char('C') => true,
        _ => false,
    }
}

fn normalized_ctrl_char_event(event: KeyEvent) -> Option<KeyEvent> {
    if !event.modifiers.contains(KeyModifiers::CONTROL)
        || event.modifiers.contains(KeyModifiers::ALT)
        || !event.modifiers.contains(KeyModifiers::SHIFT)
    {
        return None;
    }

    let KeyCode::Char(ch) = event.code else {
        return None;
    };
    if !ch.is_ascii_alphabetic() {
        return None;
    }

    Some(KeyEvent {
        code: KeyCode::Char(ch.to_ascii_lowercase()),
        modifiers: KeyModifiers::CONTROL,
    })
}
