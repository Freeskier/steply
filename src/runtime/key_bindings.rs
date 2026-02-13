use crate::runtime::command::Command;
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
    bindings: HashMap<KeyBinding, Command>,
}

impl KeyBindings {
    pub fn new() -> Self {
        let mut manager = Self::default();
        manager.install_defaults();
        manager
    }

    pub fn bind(&mut self, key: KeyBinding, command: Command) {
        self.bindings.insert(key, command);
    }

    pub fn unbind(&mut self, key: &KeyBinding) {
        self.bindings.remove(key);
    }

    pub fn resolve(&self, event: KeyEvent) -> Option<Command> {
        self.bindings.get(&KeyBinding::from_event(event)).cloned()
    }

    fn install_defaults(&mut self) {
        self.bind(KeyBinding::ctrl(KeyCode::Char('c')), Command::Exit);
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('o')),
            Command::OpenOverlayShortcut,
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('1')),
            Command::OpenOverlayAtIndex(0),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('2')),
            Command::OpenOverlayAtIndex(1),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('3')),
            Command::OpenOverlayAtIndex(2),
        );

        self.bind(
            KeyBinding::alt(KeyCode::Char('1')),
            Command::OpenOverlayAtIndex(0),
        );
        self.bind(
            KeyBinding::alt(KeyCode::Char('2')),
            Command::OpenOverlayAtIndex(1),
        );
        self.bind(
            KeyBinding::alt(KeyCode::Char('3')),
            Command::OpenOverlayAtIndex(2),
        );
        self.bind(KeyBinding::key(KeyCode::Esc), Command::Cancel);
        self.bind(KeyBinding::key(KeyCode::Tab), Command::NextFocus);
        self.bind(
            KeyBinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            Command::PrevFocus,
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Backspace),
            Command::TextAction(TextAction::DeleteWordLeft),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Char('w')),
            Command::TextAction(TextAction::DeleteWordLeft),
        );
        self.bind(
            KeyBinding::ctrl(KeyCode::Delete),
            Command::TextAction(TextAction::DeleteWordRight),
        );
    }
}
