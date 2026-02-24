use crate::core::value::Value;
use crate::runtime::event::WidgetAction;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::traits::{
    CompletionState, Drawable, InteractionResult, Interactive, RenderContext, TextAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEscBehavior {
    Hide,
    Blur,
}

#[derive(Debug, Clone)]
pub struct FilterEditOutcome {
    pub result: InteractionResult,
    pub query_changed: bool,
}

impl FilterEditOutcome {
    pub fn refresh_if_changed(self, refresh: impl FnOnce()) -> InteractionResult {
        if self.query_changed {
            refresh();
            return InteractionResult::handled();
        }
        self.result
    }
}

#[derive(Debug, Clone)]
pub enum FilterKeyOutcome {
    Ignored,
    Hide,
    Blur,
    Edited(FilterEditOutcome),
}

pub struct FilterController {
    input: TextInput,
    visible: bool,
    focused: bool,
}

impl FilterController {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            input: TextInput::new(id, ""),
            visible: false,
            focused: false,
        }
    }

    pub fn id(&self) -> &str {
        self.input.id()
    }

    pub fn input(&self) -> &TextInput {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut TextInput {
        &mut self.input
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn query(&self) -> String {
        self.input
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.input.set_value(Value::Text(String::new()));
    }

    pub fn toggle_visibility(&mut self, clear_on_hide: bool) -> bool {
        toggle_visibility(
            &mut self.input,
            &mut self.visible,
            &mut self.focused,
            clear_on_hide,
        )
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        esc_behavior: FilterEscBehavior,
    ) -> FilterKeyOutcome {
        handle_key(&mut self.input, key, esc_behavior)
    }

    pub fn handle_text_action(&mut self, action: TextAction) -> InteractionResult {
        handle_text_action(&mut self.input, action)
    }

    pub fn handle_key_with_change(
        &mut self,
        key: KeyEvent,
        esc_behavior: FilterEscBehavior,
    ) -> FilterKeyOutcome {
        self.handle_key(key, esc_behavior)
    }

    pub fn handle_text_action_with_change(&mut self, action: TextAction) -> FilterEditOutcome {
        let before = self.query();
        let result = self.handle_text_action(action);
        FilterEditOutcome {
            query_changed: self.query() != before,
            result,
        }
    }

    pub fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.input.completion()
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        self.input.cursor_pos()
    }
}

pub fn render_filter_line(
    filter: &FilterController,
    ctx: &RenderContext,
    focused: bool,
) -> SpanLine {
    render_filter_line_with(filter, ctx, focused, |ctx, focused_id| {
        ctx.with_focus(focused_id)
    })
}

pub fn render_filter_line_with(
    filter: &FilterController,
    ctx: &RenderContext,
    focused: bool,
    build_ctx: impl FnOnce(&RenderContext, Option<String>) -> RenderContext,
) -> SpanLine {
    let filter_ctx = build_ctx(
        ctx,
        if focused && filter.is_focused() {
            Some(filter.id().to_string())
        } else {
            None
        },
    );
    let mut line = vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
    line.extend(
        filter
            .input()
            .draw(&filter_ctx)
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
    );
    line
}

pub fn toggle_visibility(
    filter: &mut TextInput,
    visible: &mut bool,
    focused: &mut bool,
    clear_on_hide: bool,
) -> bool {
    *visible = !*visible;
    if *visible {
        *focused = true;
        return true;
    }

    *focused = false;
    if clear_on_hide {
        filter.set_value(crate::core::value::Value::Text(String::new()));
    }
    false
}

pub fn handle_key(
    filter: &mut TextInput,
    key: KeyEvent,
    esc_behavior: FilterEscBehavior,
) -> FilterKeyOutcome {
    if key.modifiers != KeyModifiers::NONE {
        return FilterKeyOutcome::Ignored;
    }

    let before = current_query(filter);
    match key.code {
        KeyCode::Esc => match esc_behavior {
            FilterEscBehavior::Hide => FilterKeyOutcome::Hide,
            FilterEscBehavior::Blur => FilterKeyOutcome::Blur,
        },
        KeyCode::Enter | KeyCode::Down => FilterKeyOutcome::Blur,
        _ => {
            let result = sanitize_interaction_result(filter.on_key(key));
            let query_changed = current_query(filter) != before;
            FilterKeyOutcome::Edited(FilterEditOutcome {
                result,
                query_changed,
            })
        }
    }
}

pub fn handle_text_action(filter: &mut TextInput, action: TextAction) -> InteractionResult {
    sanitize_interaction_result(filter.on_text_action(action))
}

pub fn sanitize_interaction_result(mut result: InteractionResult) -> InteractionResult {
    result
        .actions
        .retain(|action| !matches!(action, WidgetAction::InputDone));
    if result.handled {
        result.request_render = true;
    }
    result
}

fn current_query(filter: &TextInput) -> String {
    filter
        .value()
        .and_then(|value| value.to_text_scalar())
        .unwrap_or_default()
}
