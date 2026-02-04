use crate::core::component::{Component, ComponentItem};
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
    Radio,
}

pub struct SelectComponent {
    id: String,
    label: Option<String>,
    options: Vec<String>,
    mode: SelectMode,
    selected: Vec<usize>,
    active_index: usize,
}

impl SelectComponent {
    pub fn new(id: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            id: id.into(),
            label: None,
            options,
            mode: SelectMode::Single,
            selected: Vec::new(),
            active_index: 0,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_mode(mut self, mode: SelectMode) -> Self {
        self.mode = mode;
        if self.mode == SelectMode::Radio && self.selected.is_empty() && !self.options.is_empty() {
            self.selected.push(0);
        }
        self
    }

    pub fn with_selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected
    }

    pub fn value(&self) -> Vec<String> {
        let mut values = Vec::new();
        for idx in &self.selected {
            if let Some(value) = self.options.get(*idx) {
                values.push(value.clone());
            }
        }
        values
    }

    pub fn toggle(&mut self, index: usize) {
        if index >= self.options.len() {
            return;
        }

        match self.mode {
            SelectMode::Multi => {
                if let Some(pos) = self.selected.iter().position(|i| *i == index) {
                    self.selected.remove(pos);
                } else {
                    self.selected.push(index);
                }
            }
            SelectMode::Single => {
                if self.selected.iter().any(|i| *i == index) {
                    self.selected.clear();
                } else {
                    self.selected.clear();
                    self.selected.push(index);
                }
            }
            SelectMode::Radio => {
                if !self.selected.iter().any(|i| *i == index) {
                    self.selected.clear();
                    self.selected.push(index);
                }
            }
        }
    }

    fn marker(&self, index: usize) -> &'static str {
        let is_selected = self.selected.iter().any(|i| *i == index);
        match self.mode {
            SelectMode::Multi | SelectMode::Single => {
                if is_selected { "◼" } else { "◻" }
            }
            SelectMode::Radio => {
                if is_selected { "●" } else { "◌" }
            }
        }
    }

    fn move_active(&mut self, delta: isize) -> bool {
        if self.options.is_empty() {
            return false;
        }

        let len = self.options.len() as isize;
        let current = self.active_index as isize;
        let next = (current + delta + len) % len;
        let next = next as usize;

        if next == self.active_index {
            return false;
        }

        self.active_index = next;
        true
    }

    fn activate_current(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }

        let before = self.selected.clone();
        self.toggle(self.active_index);
        self.selected != before
    }
}

impl Component for SelectComponent {
    fn id(&self) -> &str {
        &self.id
    }

    fn node_ids(&self) -> &[NodeId] {
        &[]
    }

    fn nodes(&mut self) -> Vec<(NodeId, Node)> {
        Vec::new()
    }

    fn items(&self, _registry: &NodeRegistry) -> Vec<ComponentItem> {
        let mut items = Vec::new();

        if let Some(label) = &self.label {
            items.push(ComponentItem::Text(label.clone()));
        }

        for (idx, option) in self.options.iter().enumerate() {
            let cursor = if idx == self.active_index { "➤" } else { " " };
            let line = format!("{} {} {}", cursor, self.marker(idx), option);
            let active = idx == self.active_index;
            items.push(ComponentItem::Option { text: line, active });
        }

        items
    }

    fn handle_key(&mut self, code: crate::terminal::KeyCode, modifiers: crate::terminal::KeyModifiers) -> bool {
        if modifiers != crate::terminal::KeyModifiers::NONE {
            return false;
        }

        match code {
            crate::terminal::KeyCode::Up => self.move_active(-1),
            crate::terminal::KeyCode::Down => self.move_active(1),
            crate::terminal::KeyCode::Char(' ') => {
                let _ = self.activate_current();
                !self.options.is_empty()
            }
            _ => false,
        }
    }
}
