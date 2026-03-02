use crate::core::value::Value;
use crate::runtime::event::WidgetAction;
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::shared::keymap;
use crate::widgets::traits::{
    CompletionState, Drawable, InteractionResult, Interactive, RenderContext, TextAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEscBehavior {
    Hide,
    Blur,
}

#[derive(Debug, Clone)]
pub struct ListFilterUpdate {
    pub result: InteractionResult,
    pub query_changed: bool,
    pub hidden: bool,
    pub blurred: bool,
}

impl ListFilterUpdate {
    pub fn refresh_if_changed(self, refresh: impl FnOnce()) -> InteractionResult {
        if self.query_changed {
            refresh();
            return InteractionResult::handled();
        }
        self.result
    }
}

pub struct ListFilter {
    input: TextInput,
    visible: bool,
    focused: bool,
    esc_behavior: FilterEscBehavior,
    clear_on_hide: bool,
}

impl ListFilter {
    pub fn new(
        id: impl Into<String>,
        esc_behavior: FilterEscBehavior,
        clear_on_hide: bool,
    ) -> Self {
        Self {
            input: TextInput::new(id, ""),
            visible: false,
            focused: false,
            esc_behavior,
            clear_on_hide,
        }
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

    pub fn completion(&mut self) -> Option<CompletionState<'_>> {
        self.input.completion()
    }

    pub fn cursor_pos(&self) -> Option<CursorPos> {
        self.input.cursor_pos()
    }

    pub fn draw_line(&self, ctx: &RenderContext, focused: bool) -> SpanLine {
        self.draw_line_with(ctx, focused, |ctx, focused_id| ctx.with_focus(focused_id))
    }

    pub fn draw_line_with(
        &self,
        ctx: &RenderContext,
        focused: bool,
        build_ctx: impl FnOnce(&RenderContext, Option<String>) -> RenderContext,
    ) -> SpanLine {
        let filter_ctx = build_ctx(
            ctx,
            if focused && self.focused {
                Some(self.input.id().to_string())
            } else {
                None
            },
        );
        let mut line =
            vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
        line.extend(
            self.input
                .draw(&filter_ctx)
                .lines
                .into_iter()
                .next()
                .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
        );
        line
    }

    pub fn toggle_visibility(&mut self) -> ListFilterUpdate {
        let before = self.query();
        self.visible = !self.visible;
        if self.visible {
            self.focused = true;
            return ListFilterUpdate {
                result: InteractionResult::handled(),
                query_changed: self.query() != before,
                hidden: false,
                blurred: false,
            };
        }

        self.focused = false;
        if self.clear_on_hide {
            self.clear();
        }

        ListFilterUpdate {
            result: InteractionResult::handled(),
            query_changed: self.query() != before,
            hidden: true,
            blurred: false,
        }
    }

    pub fn handle_toggle_shortcut(&mut self, key: KeyEvent) -> Option<ListFilterUpdate> {
        if keymap::is_ctrl_char(key, 'f') {
            return Some(self.toggle_visibility());
        }
        None
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ListFilterUpdate {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return ListFilterUpdate {
                result: InteractionResult::ignored(),
                query_changed: false,
                hidden: false,
                blurred: false,
            };
        }

        let before = self.query();
        match key.code {
            KeyCode::Esc if keymap::has_no_modifiers(key) => match self.esc_behavior {
                FilterEscBehavior::Hide => self.toggle_visibility(),
                FilterEscBehavior::Blur => {
                    self.focused = false;
                    ListFilterUpdate {
                        result: InteractionResult::handled(),
                        query_changed: false,
                        hidden: false,
                        blurred: true,
                    }
                }
            },
            KeyCode::Enter | KeyCode::Down if keymap::has_no_modifiers(key) => {
                self.focused = false;
                ListFilterUpdate {
                    result: InteractionResult::handled(),
                    query_changed: false,
                    hidden: false,
                    blurred: true,
                }
            }
            _ => {
                let result = sanitize_interaction_result(self.input.on_key(key));
                ListFilterUpdate {
                    query_changed: self.query() != before,
                    result,
                    hidden: false,
                    blurred: false,
                }
            }
        }
    }

    pub fn handle_text_action(&mut self, action: TextAction) -> ListFilterUpdate {
        let before = self.query();
        ListFilterUpdate {
            result: sanitize_interaction_result(self.input.on_text_action(action)),
            query_changed: self.query() != before,
            hidden: false,
            blurred: false,
        }
    }
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
