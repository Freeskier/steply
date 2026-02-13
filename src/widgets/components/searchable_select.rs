use super::select_list::{SelectList, SelectMode, SelectOption};
use crate::core::search::fuzzy::ranked_matches;
use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::inputs::{text::TextInput, text_edit};
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, TextAction,
};

pub struct SearchableSelect {
    base: WidgetBase,
    query: TextInput,
    source_options: Vec<String>,
    list: SelectList,
    focus: SearchFocus,
}

impl SearchableSelect {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        let id = id.into();
        let label = label.into();
        let mut component = Self {
            base: WidgetBase::new(id.clone(), label.clone()),
            query: TextInput::new(format!("{id}__query"), label),
            source_options: options.clone(),
            list: SelectList::from_strings(format!("{id}__list"), "", options)
                .with_show_label(false),
            focus: SearchFocus::Query,
        };
        component.recompute();
        component
    }

    pub fn with_mode(mut self, mode: SelectMode) -> Self {
        self.list.set_mode(mode);
        self
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.list.set_max_visible(max_visible);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.list.set_submit_target(Some(target.into()));
        self
    }

    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.set_options(options);
        self
    }

    pub fn set_options(&mut self, options: Vec<String>) {
        self.source_options = options;
        self.recompute();
    }

    fn recompute(&mut self) {
        let query_value = self.query_value();
        let query = query_value.trim();
        if query.is_empty() {
            self.list.set_options(
                self.source_options
                    .iter()
                    .cloned()
                    .map(SelectOption::plain)
                    .collect(),
            );
            return;
        }

        let matches = ranked_matches(query, self.source_options.as_slice());
        let options = matches
            .into_iter()
            .filter_map(|entry| {
                self.source_options
                    .get(entry.index)
                    .map(|text| SelectOption::Highlighted {
                        text: text.clone(),
                        highlights: entry.ranges,
                    })
            })
            .collect::<Vec<_>>();
        self.list.set_options(options);
    }

    fn query_value(&self) -> String {
        self.query
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    fn set_query_value(&mut self, value: String) {
        self.query.set_value(Value::Text(value));
    }

    fn handle_query_key(&mut self, key: KeyEvent) -> InteractionResult {
        let result = self.query.on_key(key);
        if result.handled {
            self.recompute();
        }
        result
    }

    fn handle_delete_query_char(&mut self) -> InteractionResult {
        let Some(state) = self.query.text_editing() else {
            return InteractionResult::ignored();
        };
        if !text_edit::delete_char(state.value, state.cursor) {
            return InteractionResult::ignored();
        }
        self.recompute();
        InteractionResult::handled()
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
}

// SearchableSelect owns query and list as typed fields. They are internal
// implementation details — validation is handled by the component itself via
// validate(), so children() returns empty.
impl Component for SearchableSelect {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl Drawable for SearchableSelect {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());
        let query_ctx = self.child_context(
            ctx,
            if focused && self.focus == SearchFocus::Query {
                Some(self.query.id().to_string())
            } else {
                None
            },
        );
        let list_ctx = self.child_context(
            ctx,
            if focused && self.focus == SearchFocus::List {
                Some(self.list.id().to_string())
            } else {
                None
            },
        );

        let mut lines = self.query.draw(&query_ctx).lines;

        if focused && self.query_value().is_empty() {
            lines.push(vec![
                Span::styled(
                    "  Type to filter. Up/Down navigate.",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]);
        }

        lines.extend(self.list.draw(&list_ctx).lines);
        DrawOutput { lines }
    }
}

impl Interactive for SearchableSelect {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.focus = SearchFocus::List;
                self.list.on_key(key)
            }
            KeyCode::Enter => {
                if self.list.is_empty() {
                    return InteractionResult::handled();
                }
                self.list.on_key(key)
            }
            KeyCode::Char(' ') => {
                if self.focus == SearchFocus::List {
                    return self.list.on_key(key);
                }
                self.focus = SearchFocus::Query;
                self.handle_query_key(key)
            }
            KeyCode::Char(ch) => {
                if ch.is_control() {
                    return InteractionResult::ignored();
                }
                self.focus = SearchFocus::Query;
                self.handle_query_key(key)
            }
            KeyCode::Backspace => {
                self.focus = SearchFocus::Query;
                self.handle_query_key(key)
            }
            KeyCode::Delete => {
                self.focus = SearchFocus::Query;
                self.handle_delete_query_char()
            }
            KeyCode::Left | KeyCode::Right => {
                self.focus = SearchFocus::Query;
                self.handle_query_key(key)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        self.focus = SearchFocus::Query;
        let result = self.query.on_text_action(action);
        if result.handled {
            self.recompute();
        }
        result
    }

    fn value(&self) -> Option<Value> {
        self.list.value()
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar() {
            self.set_query_value(text.clone());
            self.recompute();
            self.list.set_value(Value::Text(text));
        } else if value.as_list().is_some() {
            self.list.set_value(value);
        }
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueChanged { change } if change.target.as_str() == self.base.id() => {
                self.set_value(change.value.clone());
                InteractionResult::handled()
            }
            _ => {
                let mut merged = InteractionResult::ignored();
                let query_result = self.query.on_event(event);
                if query_result.handled {
                    self.recompute();
                }
                merged.merge(query_result);
                merged.merge(self.list.on_event(event));
                merged
            }
        }
    }

    fn cursor_pos(&self) -> Option<crate::terminal::CursorPos> {
        if self.focus == SearchFocus::Query {
            return self.query.cursor_pos();
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchFocus {
    Query,
    List,
}
