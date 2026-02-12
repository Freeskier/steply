use crate::core::value::Value;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::ComponentBase;
use crate::widgets::inputs::text_edit;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct FilterSelect {
    base: ComponentBase,
    query: String,
    cursor: usize,
    options: Vec<String>,
    filtered: Vec<usize>,
    selected: usize,
    submit_target: Option<String>,
    max_visible: usize,
}

impl FilterSelect {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        let mut component = Self {
            base: ComponentBase::new(id, label),
            query: String::new(),
            cursor: 0,
            options,
            filtered: Vec::new(),
            selected: 0,
            submit_target: None,
            max_visible: 5,
        };
        component.recompute_filter();
        component
    }

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
        self
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible.max(1);
        self
    }

    fn recompute_filter(&mut self) {
        self.filtered.clear();

        let needle = self.query.to_lowercase();
        for (idx, option) in self.options.iter().enumerate() {
            if needle.is_empty() || option.to_lowercase().contains(&needle) {
                self.filtered.push(idx);
            }
        }

        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    fn selected_value(&self) -> Option<&str> {
        let option_idx = *self.filtered.get(self.selected)?;
        self.options.get(option_idx).map(String::as_str)
    }

    fn move_selection(&mut self, direction: isize) -> bool {
        if self.filtered.is_empty() {
            return false;
        }

        let len = self.filtered.len() as isize;
        let current = self.selected as isize;
        self.selected = ((current + direction + len) % len) as usize;
        true
    }
}

impl Drawable for FilterSelect {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());

        let mut lines = Vec::new();
        lines.push(vec![
            Span::new(format!(
                "{} {}: ",
                self.base.focus_marker(focused),
                self.base.label()
            ))
            .no_wrap(),
            Span::styled(self.query.clone(), Style::default()).no_wrap(),
        ]);

        let helper_color = if focused {
            Color::DarkGrey
        } else {
            Color::Black
        };
        lines.push(vec![
            Span::styled(
                "  Type to filter. Up/Down select.",
                Style::new().color(helper_color),
            )
            .no_wrap(),
        ]);

        if self.filtered.is_empty() {
            lines.push(vec![
                Span::styled("  (no matches)", Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        } else {
            for (row, option_idx) in self.filtered.iter().take(self.max_visible).enumerate() {
                let selected = row == self.selected;
                let marker = if selected { "  > " } else { "    " };
                let style = if selected {
                    Style::new().color(Color::Cyan).bold()
                } else {
                    Style::default()
                };
                lines.push(vec![
                    Span::new(marker).no_wrap(),
                    Span::styled(self.options[*option_idx].clone(), style).no_wrap(),
                ]);
            }
        }

        DrawOutput { lines }
    }
}

impl Interactive for FilterSelect {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(ch) => {
                text_edit::insert_char(&mut self.query, &mut self.cursor, ch);
                self.recompute_filter();
                InteractionResult::handled()
            }
            KeyCode::Backspace => {
                if text_edit::backspace_char(&mut self.query, &mut self.cursor) {
                    self.recompute_filter();
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Left => {
                if text_edit::move_left(&mut self.cursor, &self.query) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right => {
                if text_edit::move_right(&mut self.cursor, &self.query) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Up => {
                if self.move_selection(-1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Down => {
                if self.move_selection(1) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Enter => {
                if let (Some(target), Some(selected)) =
                    (self.submit_target.as_ref(), self.selected_value())
                {
                    return InteractionResult::with_event(WidgetEvent::ValueProduced {
                        target: target.clone(),
                        value: Value::Text(selected.to_string()),
                    });
                }
                InteractionResult::with_event(WidgetEvent::RequestSubmit)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        match event {
            WidgetEvent::ValueProduced { target, value } if target == self.base.id() => {
                if let Value::Text(v) = value {
                    self.query = v.clone();
                    self.cursor = text_edit::char_count(&self.query);
                    self.recompute_filter();
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        self.selected_value()
            .map(|value| Value::Text(value.to_string()))
    }

    fn set_value(&mut self, value: Value) {
        if let Value::Text(v) = value {
            self.query = v;
            self.cursor = text_edit::char_count(&self.query);
            self.recompute_filter();
        }
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let prefix = format!("{} {}: ", self.base.focus_marker(true), self.base.label());
        let mut query_width = 0usize;
        for ch in self
            .query
            .chars()
            .take(text_edit::clamp_cursor(self.cursor, &self.query))
        {
            query_width = query_width.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        }
        Some(CursorPos {
            col: (UnicodeWidthStr::width(prefix.as_str()) + query_width) as u16,
            row: 0,
        })
    }
}
