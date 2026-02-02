use crate::event::Action;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    pub fn from_key_event(event: &KeyEvent) -> Self {
        Self {
            code: event.code,
            modifiers: event.modifiers,
        }
    }
}

pub struct ActionBindings {
    bindings: HashMap<KeyBinding, Action>,
}

impl ActionBindings {
    pub fn new() -> Self {
        let mut manager = Self {
            bindings: HashMap::new(),
        };
        manager.setup_default_bindings();
        manager
    }

    fn setup_default_bindings(&mut self) {
        self.bind(KeyBinding::ctrl(KeyCode::Char('c')), Action::Exit);

        self.bind(KeyBinding::key(KeyCode::Tab), Action::NextInput);
        self.bind(
            KeyBinding::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            Action::PrevInput,
        );

        self.bind(KeyBinding::ctrl(KeyCode::Backspace), Action::DeleteWord);
        self.bind(KeyBinding::ctrl(KeyCode::Char('w')), Action::DeleteWord);
        self.bind(KeyBinding::ctrl(KeyCode::Delete), Action::DeleteWordForward);
    }

    pub fn bind(&mut self, key: KeyBinding, action: Action) {
        self.bindings.insert(key, action);
    }

    pub fn unbind(&mut self, key: &KeyBinding) {
        self.bindings.remove(key);
    }

    pub fn handle_key(&self, key_event: &KeyEvent) -> Option<Action> {
        let binding = KeyBinding::from_key_event(key_event);
        self.bindings.get(&binding).cloned()
    }
}

impl Default for ActionBindings {
    fn default() -> Self {
        Self::new()
    }
}
