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
        self.bindings.get(&KeyBinding::from_event(event)).cloned()
    }

    fn install_defaults(&mut self) {
        self.bind(KeyBinding::ctrl(KeyCode::Char('c')), Intent::Exit);
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('o')),
            Intent::OpenOverlayShortcut,
        );
        // Toggle help/hints: support multiple keyboard layouts/terminal encodings.
        self.bind(KeyBinding::ctrl(KeyCode::Char('/')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('?')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('_')), Intent::ToggleHints);
        self.bind(KeyBinding::ctrl(KeyCode::Char('7')), Intent::ToggleHints);
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
        // Toggle completion menu/ghost for focused input.
        self.bind(
            KeyBinding::ctrl(KeyCode::Char(' ')),
            Intent::ToggleCompletion,
        );
        self.bind(
            KeyBinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            Intent::CompletePrev,
        );
        self.bind(KeyBinding::alt(KeyCode::Down), Intent::NextFocus);
        self.bind(KeyBinding::alt(KeyCode::Up), Intent::PrevFocus);
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
    }
}
