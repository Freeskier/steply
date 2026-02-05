use crate::core::binding::BindTarget;
use crate::core::component::{Component, ComponentBase, EventContext, FocusMode};
use crate::core::value::Value;
use crate::ui::render::{RenderContext, RenderLine};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
    Radio,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectOption {
    Plain(String),
    Highlighted {
        text: String,
        highlights: Vec<(usize, usize)>,
    },
    Styled {
        text: String,
        highlights: Vec<(usize, usize)>,
        style: Style,
    },
}

pub struct SelectComponent {
    base: ComponentBase,
    label: Option<String>,
    options: Vec<SelectOption>,
    mode: SelectMode,
    selected: Vec<usize>,
    active_index: usize,
    scroll_offset: usize,
    max_visible: Option<usize>,
    bound_target: Option<BindTarget>,
}

impl SelectComponent {
    pub fn new(id: impl Into<String>, options: Vec<String>) -> Self {
        let options = options
            .into_iter()
            .map(SelectOption::Plain)
            .collect::<Vec<_>>();
        Self {
            base: ComponentBase::new(id),
            label: None,
            options,
            mode: SelectMode::Single,
            selected: Vec::new(),
            active_index: 0,
            scroll_offset: 0,
            max_visible: None,
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

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.set_max_visible(max_visible);
        self
    }

    pub fn set_max_visible(&mut self, max_visible: usize) {
        if max_visible == 0 {
            self.max_visible = None;
        } else {
            self.max_visible = Some(max_visible);
        }
        self.scroll_offset = 0;
        self.clamp_active();
    }

    pub fn with_options(mut self, options: Vec<SelectOption>) -> Self {
        self.set_options(options);
        self
    }

    pub fn set_options(&mut self, options: Vec<SelectOption>) {
        self.apply_options(options);
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
            if let Some(option) = self.options.get(*idx) {
                values.push(option_text(option).to_string());
            }
        }
        values
    }

    pub fn reset_active(&mut self) {
        self.active_index = 0;
        self.scroll_offset = 0;
    }

    pub fn set_active_index(&mut self, index: usize) {
        if self.options.is_empty() {
            self.active_index = 0;
            self.scroll_offset = 0;
        } else {
            self.active_index = index.min(self.options.len() - 1);
        }
        self.ensure_visible();
    }

    pub fn options(&self) -> &[SelectOption] {
        &self.options
    }

    pub fn active_index(&self) -> usize {
        self.active_index
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
        self.ensure_visible();
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

    fn apply_options(&mut self, new_options: Vec<SelectOption>) {
        let selected_values: HashSet<String> = self
            .selected
            .iter()
            .filter_map(|idx| self.options.get(*idx))
            .map(|option| option_text(option).to_string())
            .collect();

        self.options = new_options;
        self.selected = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(idx, option)| {
                if selected_values.contains(option_text(option)) {
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
            self.scroll_offset = 0;
        } else if self.active_index >= self.options.len() {
            self.active_index = self.options.len() - 1;
        }

        self.ensure_visible();
    }

    fn clamp_active(&mut self) {
        if self.options.is_empty() {
            self.active_index = 0;
        } else if self.active_index >= self.options.len() {
            self.active_index = self.options.len() - 1;
        }
    }

    fn ensure_visible(&mut self) {
        let Some(max_visible) = self.max_visible else {
            return;
        };
        let total = self.options.len();
        if total <= max_visible {
            self.scroll_offset = 0;
            return;
        }

        let max_start = total.saturating_sub(max_visible);
        if self.active_index < self.scroll_offset {
            self.scroll_offset = self.active_index;
        } else if self.active_index >= self.scroll_offset + max_visible {
            self.scroll_offset = self.active_index.saturating_sub(max_visible - 1);
        }
        if self.scroll_offset > max_start {
            self.scroll_offset = max_start;
        }
    }

    fn visible_range(&self) -> (usize, usize) {
        let total = self.options.len();
        let Some(max_visible) = self.max_visible else {
            return (0, total);
        };
        if total <= max_visible {
            return (0, total);
        }
        let start = self.scroll_offset.min(total);
        let end = (start + max_visible).min(total);
        (start, end)
    }
}

fn option_text(option: &SelectOption) -> &str {
    match option {
        SelectOption::Plain(text) => text,
        SelectOption::Highlighted { text, .. } => text,
        SelectOption::Styled { text, .. } => text,
    }
}

fn option_highlights(option: &SelectOption) -> &[(usize, usize)] {
    match option {
        SelectOption::Plain(_) => &[],
        SelectOption::Highlighted { highlights, .. } => highlights.as_slice(),
        SelectOption::Styled { highlights, .. } => highlights.as_slice(),
    }
}

fn option_style(option: &SelectOption) -> Option<&Style> {
    match option {
        SelectOption::Styled { style, .. } => Some(style),
        _ => None,
    }
}

fn push_styled_span(spans: &mut Vec<Span>, text: String, style: &Style) {
    if text.is_empty() {
        return;
    }
    spans.push(Span::new(text).with_style(style.clone()));
}

fn render_option_spans(
    option: &SelectOption,
    base_style: &Style,
    highlight_style: &Style,
) -> Vec<Span> {
    let text = option_text(option);
    let highlights = option_highlights(option);
    let base_style = if let Some(style) = option_style(option) {
        base_style.clone().merge(style)
    } else {
        base_style.clone()
    };

    if highlights.is_empty() {
        return vec![Span::new(text).with_style(base_style)];
    }

    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut ranges = highlights.to_vec();
    ranges.sort_by_key(|(start, _)| *start);

    let mut pos = 0;
    for (start, end) in ranges {
        let start = start.min(chars.len());
        let end = end.min(chars.len());
        if start > pos {
            let segment: String = chars[pos..start].iter().collect();
            push_styled_span(&mut spans, segment, &base_style);
        }
        if end > start {
            let segment: String = chars[start..end].iter().collect();
            let merged = base_style.clone().merge(highlight_style);
            push_styled_span(&mut spans, segment, &merged);
        }
        pos = end;
    }

    if pos < chars.len() {
        let segment: String = chars[pos..].iter().collect();
        push_styled_span(&mut spans, segment, &base_style);
    }

    spans
}

impl Component for SelectComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        let theme = ctx.theme();

        if let Some(label) = &self.label {
            lines.push(RenderLine {
                spans: vec![Span::new(label.clone())],
                cursor_offset: None,
            });
        }

        let inactive_style = theme.hint.clone();
        let marker_style = Style::new().with_color(Color::Green);
        let cursor_style = Style::new().with_color(Color::Yellow);
        let highlight_style = theme.decor_accent.clone().with_bold();

        let (start, end) = self.visible_range();
        let visible_len = end.saturating_sub(start);
        for (idx, option) in self
            .options
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_len)
        {
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

                let base_style = if self.base.focused && active {
                    theme.focused.clone()
                } else if selected {
                    marker_style.clone()
                } else {
                    inactive_style.clone()
                };
                spans.extend(render_option_spans(option, &base_style, &highlight_style));

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
                let base_style = Style::new();
                spans.extend(render_option_spans(option, &base_style, &highlight_style));
            } else {
                spans.push(Span::new(cursor).with_style(inactive_style.clone()));
                spans.push(Span::new(" ").with_style(inactive_style.clone()));
                spans.push(marker_span);
                spans.push(Span::new(" ").with_style(inactive_style.clone()));
                spans.extend(render_option_spans(
                    option,
                    &inactive_style,
                    &highlight_style,
                ));
            }

            lines.push(RenderLine {
                spans,
                cursor_offset: None,
            });
        }

        if let Some(max_visible) = self.max_visible {
            let total = self.options.len();
            if total > max_visible {
                let start = start + 1;
                let end = end;
                let can_scroll_up = start > 1;
                let can_scroll_down = end < total;
                let indicator = match (can_scroll_up, can_scroll_down) {
                    (true, true) => " ↑↓",
                    (true, false) => " ↑",
                    (false, true) => " ↓",
                    (false, false) => "",
                };
                let footer = format!("[{}-{} of {}]{}", start, end, total, indicator);
                lines.push(RenderLine {
                    spans: vec![Span::new(footer).with_style(theme.hint.clone())],
                    cursor_offset: None,
                });
            }
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
                .map(|option| Value::Text(option_text(option).to_string())),
        }
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::List(items) => {
                let options = items.into_iter().map(SelectOption::Plain).collect();
                self.apply_options(options);
            }
            Value::Text(text) => {
                let options = Self::parse_bound_value(text)
                    .into_iter()
                    .map(SelectOption::Plain)
                    .collect();
                self.apply_options(options);
            }
            _ => {}
        }
    }

    fn handle_key(
        &mut self,
        code: crate::terminal::KeyCode,
        modifiers: crate::terminal::KeyModifiers,
        _ctx: &mut EventContext,
    ) -> bool {
        if modifiers != crate::terminal::KeyModifiers::NONE {
            return false;
        }

        let handled = match code {
            crate::terminal::KeyCode::Up => self.move_active(-1),
            crate::terminal::KeyCode::Down => self.move_active(1),
            crate::terminal::KeyCode::Char(' ') => {
                if self.mode == SelectMode::List {
                    return false;
                }
                let _ = self.activate_current();
                !self.options.is_empty()
            }
            crate::terminal::KeyCode::Enter => {
                if self.mode == SelectMode::List {
                    let _ = self.activate_current();
                }
                if let Some(value) = Component::value(self) {
                    _ctx.produce(value);
                }
                true
            }
            _ => false,
        };

        handled
    }
}
