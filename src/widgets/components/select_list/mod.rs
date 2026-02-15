mod model;
mod render;
mod state;

use crate::core::value::Value;

use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use model::option_text;
use render::render_option_spans;
use state::{apply_options_preserving_selection, marker_symbol};

pub use model::{SelectMode, SelectOption};

pub struct SelectList {
    base: WidgetBase,
    options: Vec<SelectOption>,
    mode: SelectMode,
    selected: Vec<usize>,
    active_index: usize,
    scroll: ScrollState,
    submit_target: Option<String>,
    show_label: bool,
}

impl SelectList {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            options,
            mode: SelectMode::Single,
            selected: Vec::new(),
            active_index: 0,
            scroll: ScrollState::new(None),
            submit_target: None,
            show_label: true,
        }
    }

    pub fn from_strings(
        id: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self::new(
            id,
            label,
            options.into_iter().map(SelectOption::plain).collect(),
        )
    }

    pub fn with_mode(mut self, mode: SelectMode) -> Self {
        self.set_mode(mode);
        self
    }

    pub fn set_mode(&mut self, mode: SelectMode) {
        self.mode = mode;
        if self.mode == SelectMode::Radio && self.selected.is_empty() && !self.options.is_empty() {
            self.selected.push(0);
        }
    }

    pub fn with_show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.set_max_visible(max_visible);
        self
    }

    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.scroll.max_visible = if max_visible == 0 {
            None
        } else {
            Some(max_visible)
        };
        self.scroll.offset = 0;
        ScrollState::clamp_active(&mut self.active_index, self.options.len());
        self.scroll
            .ensure_visible(self.active_index, self.options.len());
    }

    pub fn with_options(mut self, options: Vec<SelectOption>) -> Self {
        self.set_options(options);
        self
    }

    pub fn set_options(&mut self, options: Vec<SelectOption>) {
        apply_options_preserving_selection(
            &mut self.options,
            &mut self.selected,
            &mut self.active_index,
            &mut self.scroll.offset,
            self.mode,
            options,
        );
        self.scroll
            .ensure_visible(self.active_index, self.options.len());
    }

    pub fn with_selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected
            .into_iter()
            .filter(|index| *index < self.options.len())
            .collect();
        if self.mode == SelectMode::Radio && self.selected.is_empty() && !self.options.is_empty() {
            self.selected.push(0);
        }
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.set_submit_target(Some(target.into()));
        self
    }

    pub fn set_submit_target(&mut self, target: Option<String>) {
        self.submit_target = target;
    }

    pub fn selected_indices(&self) -> &[usize] {
        self.selected.as_slice()
    }

    pub fn selected_values(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|index| self.options.get(*index))
            .map(|option| option_text(option).to_string())
            .collect()
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn mode(&self) -> SelectMode {
        self.mode
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    fn toggle(&mut self, index: usize) {
        if index >= self.options.len() {
            return;
        }

        match self.mode {
            SelectMode::Multi => {
                if let Some(pos) = self.selected.iter().position(|selected| *selected == index) {
                    self.selected.remove(pos);
                } else {
                    self.selected.push(index);
                    self.selected.sort_unstable();
                }
            }
            SelectMode::Single | SelectMode::Radio | SelectMode::List => {
                if !self.selected.iter().any(|selected| *selected == index) {
                    self.selected.clear();
                    self.selected.push(index);
                }
            }
        }
    }

    fn move_active(&mut self, delta: isize) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let len = self.options.len() as isize;
        let current = self.active_index as isize;
        let next = ((current + delta + len) % len) as usize;
        if next == self.active_index {
            return false;
        }
        self.active_index = next;
        self.scroll
            .ensure_visible(self.active_index, self.options.len());
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

    fn line_items(&self, focused: bool) -> Vec<Vec<Span>> {
        let mut lines = Vec::<Vec<Span>>::new();
        let inactive_style = Style::new().color(Color::DarkGrey);
        let marker_selected_style = Style::new().color(Color::Green);
        let cursor_style = Style::new().color(Color::Yellow);
        let highlight_style = Style::new().color(Color::Yellow).bold();

        let total = self.options.len();
        let (start, end) = self.scroll.visible_range(total);

        for (index, option) in self
            .options
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            let active = index == self.active_index;
            let selected = self.selected.iter().any(|entry| *entry == index);
            let cursor = if focused && active { "❯" } else { " " };

            if self.mode == SelectMode::List {
                let mut spans = Vec::<Span>::new();
                if focused && active {
                    spans.push(Span::styled(cursor, cursor_style).no_wrap());
                } else {
                    spans.push(Span::styled(cursor, inactive_style).no_wrap());
                }
                spans.push(Span::new(" ").no_wrap());

                let base_style = if focused && active {
                    Style::new().color(Color::Cyan).bold()
                } else if selected {
                    marker_selected_style
                } else {
                    inactive_style
                };
                spans.extend(render_option_spans(option, base_style, highlight_style));
                lines.push(spans);
                continue;
            }

            let marker = marker_symbol(self.mode, self.selected.as_slice(), index);
            let marker_span = if selected {
                Span::styled(marker, marker_selected_style).no_wrap()
            } else if active {
                Span::new(marker).no_wrap()
            } else {
                Span::styled(marker, inactive_style).no_wrap()
            };

            let mut spans = Vec::<Span>::new();
            if active {
                spans.push(Span::styled(cursor, cursor_style).no_wrap());
                spans.push(Span::new(" ").no_wrap());
                spans.push(marker_span);
                spans.push(Span::new(" ").no_wrap());
                spans.extend(render_option_spans(
                    option,
                    Style::default(),
                    highlight_style,
                ));
            } else {
                spans.push(Span::styled(cursor, inactive_style).no_wrap());
                spans.push(Span::styled(" ", inactive_style).no_wrap());
                spans.push(marker_span);
                spans.push(Span::styled(" ", inactive_style).no_wrap());
                spans.extend(render_option_spans(option, inactive_style, highlight_style));
            }
            lines.push(spans);
        }

        if let Some(text) = self.scroll.footer(total) {
            lines.push(vec![
                Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        lines
    }
}

impl Component for SelectList {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl Drawable for SelectList {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());

        let mut lines = Vec::<Vec<Span>>::new();
        if self.show_label && !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }
        lines.extend(self.line_items(focused));
        DrawOutput { lines }
    }
}

impl Interactive for SelectList {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Up => {
                if self.move_active(-1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Down => {
                if self.move_active(1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Char(' ') => {
                if self.mode == SelectMode::List {
                    return InteractionResult::ignored();
                }
                let _ = self.activate_current();
                InteractionResult::handled()
            }
            KeyCode::Enter => {
                if self.mode == SelectMode::List {
                    let _ = self.activate_current();
                }

                let Some(value) = self.value() else {
                    return InteractionResult::submit_requested();
                };
                InteractionResult::submit_or_produce(self.submit_target.as_deref(), value)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        if self.options.is_empty() {
            return None;
        }

        match self.mode {
            SelectMode::Multi => Some(Value::List(
                self.selected_values()
                    .into_iter()
                    .map(Value::Text)
                    .collect::<Vec<_>>(),
            )),
            SelectMode::Single | SelectMode::Radio | SelectMode::List => self
                .selected
                .first()
                .and_then(|index| self.options.get(*index))
                .map(|option| Value::Text(option_text(option).to_string())),
        }
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar() {
            if let Some(index) = self
                .options
                .iter()
                .position(|option| option_text(option) == text.as_str())
            {
                self.selected.clear();
                self.selected.push(index);
                self.active_index = index;
            }
        } else if let Some(values) = value.as_list() {
            self.selected.clear();
            for value in values {
                let Some(value) = value.to_text_scalar() else {
                    continue;
                };
                if let Some(index) = self
                    .options
                    .iter()
                    .position(|option| option_text(option) == value.as_str())
                {
                    if !self.selected.iter().any(|selected| *selected == index) {
                        self.selected.push(index);
                    }
                }
            }

            self.selected.sort_unstable();
            if let Some(first) = self.selected.first().copied() {
                self.active_index = first;
            }
        }

        ScrollState::clamp_active(&mut self.active_index, self.options.len());
        self.scroll
            .ensure_visible(self.active_index, self.options.len());
    }
}
