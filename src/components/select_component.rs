use crate::core::binding::BindTarget;
use crate::core::component::{Component, ComponentBase, ComponentResponse};
use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::core::value::Value;
use crate::ui::render::RenderLine;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::ui::theme::Theme;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
    Radio,
    List,
}

pub struct SelectComponent {
    base: ComponentBase,
    label: Option<String>,
    options: Vec<String>,
    mode: SelectMode,
    selected: Vec<usize>,
    active_index: usize,
    bound_target: Option<BindTarget>,
}

impl SelectComponent {
    pub fn new(id: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: ComponentBase::new(id),
            label: None,
            options,
            mode: SelectMode::Single,
            selected: Vec::new(),
            active_index: 0,
            bound_target: None,
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

    pub fn with_bind_target(mut self, target: BindTarget) -> Self {
        self.bound_target = Some(target);
        self
    }

    pub fn bind_to_input(mut self, id: impl Into<String>) -> Self {
        self.bound_target = Some(BindTarget::Input(id.into()));
        self
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected
    }

    pub fn selected_values(&self) -> Vec<String> {
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
            SelectMode::List => {
                self.selected.clear();
                self.selected.push(index);
            }
        }
    }

    fn marker_symbol(&self, index: usize) -> &'static str {
        let is_selected = self.selected.iter().any(|i| *i == index);
        match self.mode {
            SelectMode::Multi | SelectMode::Single => {
                if is_selected {
                    "■"
                } else {
                    "□"
                }
            }
            SelectMode::Radio => {
                if is_selected {
                    "●"
                } else {
                    "○"
                }
            }
            SelectMode::List => "",
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

    fn parse_bound_value(value: String) -> Vec<String> {
        value
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }

    fn apply_options(&mut self, new_options: Vec<String>) {
        if new_options == self.options {
            return;
        }

        let selected_values: HashSet<String> = self
            .selected
            .iter()
            .filter_map(|idx| self.options.get(*idx).cloned())
            .collect();

        self.options = new_options;
        self.selected = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| {
                if selected_values.contains(value) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if self.mode == SelectMode::Radio && self.selected.is_empty() && !self.options.is_empty() {
            self.selected.push(0);
        }

        if self.options.is_empty() {
            self.active_index = 0;
        } else if self.active_index >= self.options.len() {
            self.active_index = self.options.len() - 1;
        }
    }
}

impl Component for SelectComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn node_ids(&self) -> &[NodeId] {
        &[]
    }

    fn nodes(&mut self) -> Vec<(NodeId, Node)> {
        Vec::new()
    }

    fn render(&self, _registry: &NodeRegistry, theme: &Theme) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        if let Some(label) = &self.label {
            lines.push(RenderLine {
                spans: vec![Span::new(label.clone())],
                cursor_offset: None,
            });
        }

        let inactive_style = theme.hint.clone();
        let marker_style = Style::new().with_color(Color::Green);
        let cursor_style = Style::new().with_color(Color::Yellow);

        for (idx, option) in self.options.iter().enumerate() {
            let active = idx == self.active_index;
            let selected = self.selected.iter().any(|i| *i == idx);
            let cursor = if self.base.focused && active {
                "❯"
            } else {
                " "
            };
            if self.mode == SelectMode::List {
                let mut spans = Vec::new();
                spans.push(Span::new(cursor).with_style(cursor_style.clone()));
                spans.push(Span::new(" "));

                let mut text_span = Span::new(option.clone());
                if self.base.focused && active {
                    text_span = text_span.with_style(theme.focused.clone());
                } else if selected {
                    text_span = text_span.with_style(marker_style.clone());
                } else {
                    text_span = text_span.with_style(inactive_style.clone());
                }
                spans.push(text_span);

                lines.push(RenderLine {
                    spans,
                    cursor_offset: None,
                });
                continue;
            }
            let marker = self.marker_symbol(idx);
            let marker_span = if selected {
                Span::new(marker).with_style(marker_style.clone())
            } else if active {
                Span::new(marker)
            } else {
                Span::new(marker).with_style(inactive_style.clone())
            };

            let mut spans = Vec::new();
            if active {
                spans.push(Span::new(cursor).with_style(cursor_style.clone()));
                spans.push(Span::new(" "));
                spans.push(marker_span);
                spans.push(Span::new(" "));
                spans.push(Span::new(option.clone()));
            } else {
                spans.push(Span::new(cursor).with_style(inactive_style.clone()));
                spans.push(Span::new(" ").with_style(inactive_style.clone()));
                spans.push(marker_span);
                spans.push(Span::new(" ").with_style(inactive_style.clone()));
                spans.push(Span::new(option.clone()).with_style(inactive_style.clone()));
            }

            lines.push(RenderLine {
                spans,
                cursor_offset: None,
            });
        }

        lines
    }

    fn bind_target(&self) -> Option<BindTarget> {
        self.bound_target.clone()
    }

    fn value(&self) -> Option<Value> {
        if self.options.is_empty() {
            return None;
        }

        match self.mode {
            SelectMode::Multi => Some(Value::List(self.selected_values())),
            SelectMode::Single | SelectMode::Radio | SelectMode::List => self
                .selected
                .first()
                .and_then(|idx| self.options.get(*idx))
                .cloned()
                .map(Value::Text),
        }
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::List(items) => self.apply_options(items),
            Value::Text(text) => self.apply_options(Self::parse_bound_value(text)),
            _ => {}
        }
    }

    fn handle_key(
        &mut self,
        code: crate::terminal::KeyCode,
        modifiers: crate::terminal::KeyModifiers,
    ) -> ComponentResponse {
        if modifiers != crate::terminal::KeyModifiers::NONE {
            return ComponentResponse::not_handled();
        }

        let handled = match code {
            crate::terminal::KeyCode::Up => self.move_active(-1),
            crate::terminal::KeyCode::Down => self.move_active(1),
            crate::terminal::KeyCode::Char(' ') => {
                if self.mode == SelectMode::List {
                    return ComponentResponse::not_handled();
                }
                let _ = self.activate_current();
                !self.options.is_empty()
            }
            crate::terminal::KeyCode::Enter => {
                if self.mode == SelectMode::List {
                    let _ = self.activate_current();
                }
                if let Some(value) = Component::value(self) {
                    return ComponentResponse::produced(value);
                }
                true
            }
            _ => false,
        };

        if handled {
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }
}
