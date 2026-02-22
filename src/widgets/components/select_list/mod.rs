mod model;
mod render;
mod state;

use std::sync::Arc;

use crate::core::NodeId;
use crate::core::search::fuzzy::match_text;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, TextAction,
};
use model::item_search_text;
use render::{OptionRenderer, default_option_renderer};
use state::marker_symbol;

pub use model::{SelectItem, SelectItemView, SelectMode};
pub use render::SelectItemRenderState;

pub struct SelectList {
    base: WidgetBase,
    source_options: Vec<SelectItem>,
    options: Vec<SelectItem>,
    visible_to_source: Vec<usize>,
    mode: SelectMode,
    selected: Vec<usize>,
    active_index: usize,
    scroll: ScrollState,
    submit_target: Option<ValueTarget>,
    show_label: bool,
    filter: TextInput,
    filter_visible: bool,
    filter_focus: bool,
    option_renderer: OptionRenderer,
}

impl SelectList {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<SelectItem>) -> Self {
        let id = id.into();
        let label = label.into();
        let mut this = Self {
            base: WidgetBase::new(id.clone(), label),
            source_options: options.clone(),
            options,
            visible_to_source: Vec::new(),
            mode: SelectMode::Single,
            selected: Vec::new(),
            active_index: 0,
            scroll: ScrollState::new(None),
            submit_target: None,
            show_label: true,
            filter: TextInput::new(format!("{id}__filter"), ""),
            filter_visible: false,
            filter_focus: false,
            option_renderer: default_option_renderer(),
        };
        this.apply_filter(None);
        this
    }

    pub fn from_strings(
        id: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self::new(
            id,
            label,
            options.into_iter().map(SelectItem::plain).collect(),
        )
    }

    pub fn with_mode(mut self, mode: SelectMode) -> Self {
        self.set_mode(mode);
        self
    }

    pub fn set_mode(&mut self, mode: SelectMode) {
        self.mode = mode;
        self.ensure_radio_selection();
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

    pub fn with_options(mut self, options: Vec<SelectItem>) -> Self {
        self.set_options(options);
        self
    }

    pub fn with_option_renderer<F>(mut self, renderer: F) -> Self
    where
        F: Fn(&SelectItem, SelectItemRenderState) -> Vec<Vec<Span>> + Send + Sync + 'static,
    {
        self.set_option_renderer(renderer);
        self
    }

    pub fn set_option_renderer<F>(&mut self, renderer: F)
    where
        F: Fn(&SelectItem, SelectItemRenderState) -> Vec<Vec<Span>> + Send + Sync + 'static,
    {
        self.option_renderer = Arc::new(renderer);
    }

    pub fn reset_option_renderer(&mut self) {
        self.option_renderer = default_option_renderer();
    }

    pub fn set_options(&mut self, options: Vec<SelectItem>) {
        let selected_values = self.selected_values();
        self.source_options = options;

        self.selected = self
            .source_options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                if selected_values.iter().any(|value| value == &option.value) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        self.ensure_radio_selection();

        self.apply_filter(None);
    }

    pub fn with_selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected
            .into_iter()
            .filter(|index| *index < self.source_options.len())
            .collect();
        self.ensure_radio_selection();
        self.apply_filter(None);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.set_submit_target(Some(ValueTarget::node(target)));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.set_submit_target(Some(ValueTarget::path(root, path)));
        self
    }

    pub fn set_submit_target(&mut self, target: Option<ValueTarget>) {
        self.submit_target = target;
    }

    pub fn selected_indices(&self) -> &[usize] {
        self.selected.as_slice()
    }

    pub fn selected_values(&self) -> Vec<Value> {
        self.selected
            .iter()
            .filter_map(|index| self.source_options.get(*index))
            .map(|option| option.value.clone())
            .collect()
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn set_active_index(&mut self, index: usize) {
        if self.options.is_empty() {
            self.active_index = 0;
            self.scroll.offset = 0;
            return;
        }
        self.active_index = index.min(self.options.len() - 1);
        self.scroll
            .ensure_visible(self.active_index, self.options.len());
    }

    pub fn mode(&self) -> SelectMode {
        self.mode
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    fn toggle_filter_visibility(&mut self) {
        self.filter_visible = !self.filter_visible;
        if self.filter_visible {
            self.filter_focus = true;
            return;
        }

        self.filter_focus = false;
        self.filter.set_value(Value::Text(String::new()));
        self.apply_filter(None);
    }

    fn filter_query(&self) -> String {
        self.filter
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    fn active_source_index(&self) -> Option<usize> {
        self.visible_to_source.get(self.active_index).copied()
    }

    fn ensure_radio_selection(&mut self) {
        if self.mode == SelectMode::Radio
            && self.selected.is_empty()
            && !self.source_options.is_empty()
        {
            self.selected.push(0);
        }
    }

    fn apply_filter_on_edit(
        &mut self,
        before_query: String,
        result: InteractionResult,
    ) -> InteractionResult {
        if self.filter_query() != before_query {
            self.apply_filter(None);
            return InteractionResult::handled();
        }
        result
    }

    fn apply_filter(&mut self, preferred_source: Option<usize>) {
        let preferred_source = preferred_source.or_else(|| self.active_source_index());
        let query = self.filter_query();
        let query = query.trim();

        if query.is_empty() {
            self.options = self.source_options.clone();
            self.visible_to_source = (0..self.source_options.len()).collect();
        } else {
            let (options, mapping) = fuzzy_filter_options(query, self.source_options.as_slice());
            self.options = options;
            self.visible_to_source = mapping;
        }

        self.selected
            .retain(|index| *index < self.source_options.len());
        self.ensure_radio_selection();

        if self.options.is_empty() {
            self.active_index = 0;
            self.scroll.offset = 0;
            return;
        }

        if let Some(source) = preferred_source
            && let Some(pos) = self.visible_to_source.iter().position(|idx| *idx == source)
        {
            self.active_index = pos;
        } else if let Some(pos) = self
            .selected
            .first()
            .and_then(|source| self.visible_to_source.iter().position(|idx| idx == source))
        {
            self.active_index = pos;
        } else if self.active_index >= self.options.len() {
            self.active_index = self.options.len() - 1;
        }

        self.scroll
            .ensure_visible(self.active_index, self.options.len());
    }

    fn toggle(&mut self, index: usize) {
        let Some(source_index) = self.visible_to_source.get(index).copied() else {
            return;
        };

        match self.mode {
            SelectMode::Multi => {
                if let Some(pos) = self
                    .selected
                    .iter()
                    .position(|selected| *selected == source_index)
                {
                    self.selected.remove(pos);
                } else {
                    self.selected.push(source_index);
                    self.selected.sort_unstable();
                }
            }
            SelectMode::Single | SelectMode::Radio | SelectMode::List => {
                if !self.selected.contains(&source_index) {
                    self.selected.clear();
                    self.selected.push(source_index);
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

        for index in start..end {
            let Some(option) = self.options.get(index) else {
                continue;
            };
            let active = index == self.active_index;
            let selected = self
                .visible_to_source
                .get(index)
                .is_some_and(|source| self.selected.contains(source));
            let cursor = if focused && active { "❯" } else { " " };

            if self.mode == SelectMode::List {
                let base_style = if focused && active {
                    Style::new().color(Color::Cyan).bold()
                } else if selected {
                    marker_selected_style
                } else {
                    inactive_style
                };
                let option_lines = (self.option_renderer)(
                    option,
                    SelectItemRenderState {
                        focused,
                        active,
                        selected,
                        mode: self.mode,
                        base_style,
                        highlight_style,
                    },
                );
                for (line_idx, option_line) in option_lines.into_iter().enumerate() {
                    let mut spans = Vec::<Span>::new();
                    if line_idx == 0 {
                        if focused && active {
                            spans.push(Span::styled(cursor, cursor_style).no_wrap());
                        } else {
                            spans.push(Span::styled(cursor, inactive_style).no_wrap());
                        }
                    } else {
                        spans.push(Span::styled(" ", inactive_style).no_wrap());
                    }
                    spans.push(Span::styled(" ", inactive_style).no_wrap());
                    spans.extend(option_line);
                    lines.push(spans);
                }
                continue;
            }

            let marker = marker_symbol(self.mode, selected);
            let marker_style = if selected {
                marker_selected_style
            } else if active {
                Style::default()
            } else {
                inactive_style
            };
            let base_style = if active {
                Style::default()
            } else {
                inactive_style
            };
            let option_lines = (self.option_renderer)(
                option,
                SelectItemRenderState {
                    focused,
                    active,
                    selected,
                    mode: self.mode,
                    base_style,
                    highlight_style,
                },
            );

            for (line_idx, option_line) in option_lines.into_iter().enumerate() {
                let mut spans = Vec::<Span>::new();
                if line_idx == 0 {
                    if active {
                        spans.push(Span::styled(cursor, cursor_style).no_wrap());
                        spans.push(Span::new(" ").no_wrap());
                        spans.push(Span::styled(marker, marker_style).no_wrap());
                        spans.push(Span::new(" ").no_wrap());
                    } else {
                        spans.push(Span::styled(cursor, inactive_style).no_wrap());
                        spans.push(Span::styled(" ", inactive_style).no_wrap());
                        spans.push(Span::styled(marker, marker_style).no_wrap());
                        spans.push(Span::styled(" ", inactive_style).no_wrap());
                    }
                } else {
                    spans.push(Span::styled(" ", inactive_style).no_wrap());
                    spans.push(Span::styled(" ", inactive_style).no_wrap());
                    spans.push(Span::styled(" ", inactive_style).no_wrap());
                    spans.push(Span::styled(" ", inactive_style).no_wrap());
                }
                spans.extend(option_line);
                lines.push(spans);
            }
        }

        if let Some(text) = self.scroll.footer(total) {
            lines.push(vec![
                Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        lines
    }

    fn child_context(&self, ctx: &RenderContext, focused_id: Option<String>) -> RenderContext {
        RenderContext {
            focused_id,
            terminal_size: ctx.terminal_size,
            visible_errors: ctx.visible_errors.clone(),
            invalid_hidden: ctx.invalid_hidden.clone(),
            completion_menus: ctx.completion_menus.clone(),
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Esc => {
                self.toggle_filter_visibility();
                InteractionResult::handled()
            }
            KeyCode::Enter | KeyCode::Down => {
                self.filter_focus = false;
                InteractionResult::handled()
            }
            _ => {
                let before = self.filter_query();
                let result = sanitize_child_result(self.filter.on_key(key));
                self.apply_filter_on_edit(before, result)
            }
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Up => {
                if self.filter_visible && self.active_index == 0 {
                    self.filter_focus = true;
                    return InteractionResult::handled();
                }
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
                    return InteractionResult::input_done();
                };
                InteractionResult::submit_or_produce(self.submit_target.as_ref(), value)
            }
            _ => InteractionResult::ignored(),
        }
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

        if self.filter_visible {
            let filter_ctx = self.child_context(
                ctx,
                if focused && self.filter_focus {
                    Some(self.filter.id().to_string())
                } else {
                    None
                },
            );
            let mut filter_line =
                vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
            filter_line.extend(
                self.filter
                    .draw(&filter_ctx)
                    .lines
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
            );
            lines.push(filter_line);
        }

        lines.extend(self.line_items(focused && !self.filter_focus));
        DrawOutput { lines }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }

        let mut hints = vec![
            HintItem::new("↑ ↓", "move", HintGroup::Navigation).with_priority(10),
            HintItem::new("Enter", "confirm", HintGroup::Action).with_priority(20),
            HintItem::new("Ctrl+F", "toggle filter", HintGroup::View).with_priority(30),
        ];
        if self.mode != SelectMode::List {
            hints.push(
                HintItem::new("Space", "toggle selection", HintGroup::Action).with_priority(21),
            );
        }
        if self.filter_focus {
            hints.push(HintItem::new("Esc", "close filter", HintGroup::View).with_priority(31));
        }
        hints
    }
}

impl Interactive for SelectList {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }

        if self.filter_focus {
            return self.handle_filter_key(key);
        }

        self.handle_list_key(key)
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if !self.filter_focus {
            return InteractionResult::ignored();
        }

        let before = self.filter_query();
        let result = sanitize_child_result(self.filter.on_text_action(action));
        self.apply_filter_on_edit(before, result)
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if !self.filter_focus {
            return None;
        }
        self.filter.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if !self.filter_focus {
            return None;
        }
        let local = self.filter.cursor_pos()?;
        let row = if self.show_label && !self.base.label().is_empty() {
            1
        } else {
            0
        };
        Some(CursorPos {
            col: local.col.saturating_add(8),
            row,
        })
    }

    fn value(&self) -> Option<Value> {
        if self.source_options.is_empty() {
            return None;
        }

        match self.mode {
            SelectMode::Multi => Some(Value::List(self.selected_values())),
            SelectMode::Single | SelectMode::Radio | SelectMode::List => self
                .selected
                .first()
                .and_then(|index| self.source_options.get(*index))
                .map(|option| option.value.clone()),
        }
    }

    fn set_value(&mut self, value: Value) {
        if let Some(options) = options_from_value(&value) {
            self.set_options(options);
            return;
        }

        if let Some(values) = value.as_list() {
            self.selected.clear();
            for value in values {
                if let Some(index) = self
                    .source_options
                    .iter()
                    .position(|option| option.value == *value)
                    && !self.selected.contains(&index)
                {
                    self.selected.push(index);
                }
            }
            self.selected.sort_unstable();
        } else if let Some(index) = self
            .source_options
            .iter()
            .position(|option| option.value == value)
        {
            self.selected.clear();
            self.selected.push(index);
        }

        self.apply_filter(None);
    }
}

fn sanitize_child_result(mut result: InteractionResult) -> InteractionResult {
    result
        .actions
        .retain(|action| !matches!(action, crate::runtime::event::WidgetAction::InputDone));
    if result.handled {
        result.request_render = true;
    }
    result
}

fn fuzzy_filter_options(
    query: &str,
    source_options: &[SelectItem],
) -> (Vec<SelectItem>, Vec<usize>) {
    let mut scored = Vec::<(usize, i32, SelectItem)>::new();
    for (index, option) in source_options.iter().enumerate() {
        let Some((score, option)) = highlight_item_for_query(query, option) else {
            continue;
        };
        scored.push((index, score, option));
    }

    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let mapping = scored
        .iter()
        .map(|(index, _, _)| *index)
        .collect::<Vec<_>>();
    let options = scored.into_iter().map(|(_, _, option)| option).collect();
    (options, mapping)
}

fn highlight_item_for_query(query: &str, option: &SelectItem) -> Option<(i32, SelectItem)> {
    let (search_score, _) = fuzzy_match_text(query, item_search_text(option))?;
    let mut highlighted = option.clone();
    let mut score = search_score;

    match &mut highlighted.view {
        SelectItemView::Plain { text, highlights } => {
            *highlights = fuzzy_match_text(query, text.as_str())
                .map(|(_, ranges)| ranges)
                .unwrap_or_default();
        }
        SelectItemView::Detailed {
            title,
            description,
            title_highlights,
            description_highlights,
            ..
        } => {
            let title_match = fuzzy_match_text(query, title.as_str());
            let description_match = fuzzy_match_text(query, description.as_str());
            *title_highlights = title_match
                .as_ref()
                .map(|(_, ranges)| ranges.clone())
                .unwrap_or_default();
            *description_highlights = description_match
                .as_ref()
                .map(|(_, ranges)| ranges.clone())
                .unwrap_or_default();

            let title_score = title_match.map(|(s, _)| s + 30).unwrap_or_default();
            let description_score = description_match.map(|(s, _)| s).unwrap_or_default();
            score = score.max(title_score.max(description_score));
        }
        SelectItemView::Styled {
            text, highlights, ..
        }
        | SelectItemView::Split {
            text, highlights, ..
        }
        | SelectItemView::Suffix {
            text, highlights, ..
        }
        | SelectItemView::SplitSuffix {
            text, highlights, ..
        } => {
            *highlights = fuzzy_match_text(query, text.as_str())
                .map(|(_, ranges)| ranges)
                .unwrap_or_default();
        }
    }

    Some((score, highlighted))
}

fn fuzzy_match_text(query: &str, text: &str) -> Option<(i32, Vec<(usize, usize)>)> {
    match_text(query, text)
}

fn options_from_value(value: &Value) -> Option<Vec<SelectItem>> {
    match value {
        Value::Object(map) => map.get("options").and_then(options_from_value),
        Value::List(items) if items.iter().all(|item| matches!(item, Value::Object(_))) => {
            let mut options = Vec::<SelectItem>::new();
            for item in items {
                if let Some(option) = option_from_object_value(item) {
                    options.push(option);
                }
            }
            Some(options)
        }
        _ => None,
    }
}

fn option_from_object_value(value: &Value) -> Option<SelectItem> {
    let Value::Object(map) = value else {
        return None;
    };

    let value_text = map
        .get("value")
        .and_then(Value::to_text_scalar)
        .or_else(|| map.get("id").and_then(Value::to_text_scalar));
    let title = map
        .get("title")
        .and_then(Value::to_text_scalar)
        .or_else(|| value_text.clone());
    let description = map.get("description").and_then(Value::to_text_scalar);

    let title = title?;
    let value_text = value_text.unwrap_or_else(|| title.clone());

    if let Some(description) = description {
        return Some(SelectItem::detailed(value_text, title, description));
    }
    Some(SelectItem::plain(value_text))
}
