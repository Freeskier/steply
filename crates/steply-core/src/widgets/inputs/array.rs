use crate::core::value::Value;
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::shared::text_edit;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
    StoreSyncPolicy, TextAction, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct ArrayInput {
    base: WidgetBase,
    items: Vec<String>,
    active: usize,
    cursor: usize,
    validators: Vec<Validator>,
}

impl ArrayInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            items: vec![String::new()],
            active: 0,
            cursor: 0,
            validators: Vec::new(),
        }
    }

    pub fn with_items(mut self, items: Vec<String>) -> Self {
        self.replace_items(items);
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
        self
    }

    fn normalized_items(items: Vec<String>) -> Vec<String> {
        items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    }

    fn replace_items(&mut self, items: Vec<String>) {
        let cleaned = Self::normalized_items(items);

        if cleaned.is_empty() {
            self.items = vec![String::new()];
            self.active = 0;
            self.cursor = 0;
            return;
        }

        self.items = cleaned;
        self.active = 0;
        self.cursor = text_edit::char_count(self.items[0].as_str());
    }

    fn replace_items_preserving_state(&mut self, items: Vec<String>) {
        let cleaned = Self::normalized_items(items);

        if cleaned.is_empty() {
            self.items = vec![String::new()];
            self.active = 0;
            self.cursor = 0;
            return;
        }

        let active = self.active.min(cleaned.len().saturating_sub(1));
        let cursor = self.cursor;
        self.items = cleaned;
        self.active = active;
        self.cursor = text_edit::clamp_cursor(cursor, self.items[self.active].as_str());
    }

    fn ensure_invariants(&mut self) {
        if self.items.is_empty() {
            self.items.push(String::new());
        }
        if self.active >= self.items.len() {
            self.active = self.items.len().saturating_sub(1);
        }
        if let Some(item) = self.items.get(self.active) {
            self.cursor = text_edit::clamp_cursor(self.cursor, item.as_str());
        } else {
            self.cursor = 0;
        }
    }

    fn active_item(&self) -> &str {
        self.items
            .get(self.active)
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn active_item_mut(&mut self) -> &mut String {
        self.ensure_invariants();
        &mut self.items[self.active]
    }

    fn split_active(&mut self) {
        self.ensure_invariants();
        let current = self.active_item().to_string();
        let split_at = self.cursor.min(text_edit::char_count(current.as_str()));
        let left = current.chars().take(split_at).collect::<String>();
        let right = current.chars().skip(split_at).collect::<String>();

        self.items[self.active] = left.trim().to_string();
        let insert_at = self.active + 1;
        self.items.insert(insert_at, right.trim().to_string());
        self.active = insert_at;
        self.cursor = text_edit::char_count(self.items[self.active].as_str());
        self.normalize_items();
    }

    fn normalize_items(&mut self) {
        for item in &mut self.items {
            *item = item.trim().to_string();
        }
        self.ensure_invariants();
    }

    fn backspace_at_start(&mut self) -> bool {
        self.ensure_invariants();
        if self.active == 0 {
            return false;
        }

        let current = self.items.remove(self.active);
        self.active -= 1;
        let previous_len = text_edit::char_count(self.items[self.active].as_str());
        self.cursor = previous_len;

        if !current.trim().is_empty() {
            self.items[self.active].push_str(current.as_str());
            self.cursor = text_edit::char_count(self.items[self.active].as_str());
        }
        true
    }

    fn delete_at_end(&mut self) -> bool {
        self.ensure_invariants();
        if self.active + 1 >= self.items.len() {
            return false;
        }

        let next = self.items.remove(self.active + 1);
        if !next.trim().is_empty() {
            self.items[self.active].push_str(next.as_str());
        }
        true
    }

    fn move_left_at_start(&mut self) -> bool {
        self.ensure_invariants();
        if self.active > 0 {
            self.active -= 1;
            self.cursor = text_edit::char_count(self.items[self.active].as_str());
            return true;
        }
        false
    }

    fn move_right_at_end(&mut self) -> bool {
        self.ensure_invariants();
        if self.active + 1 < self.items.len() {
            self.active += 1;
            self.cursor = 0;
            return true;
        }
        false
    }

    fn remove_active(&mut self) -> bool {
        self.ensure_invariants();
        if self.items.len() == 1 {
            self.items[0].clear();
            self.active = 0;
            self.cursor = 0;
            return true;
        }

        self.items.remove(self.active);
        if self.active >= self.items.len() {
            self.active = self.items.len() - 1;
        }
        self.cursor = text_edit::char_count(self.items[self.active].as_str());
        true
    }

    fn remove_next(&mut self) -> bool {
        self.ensure_invariants();
        if self.active + 1 >= self.items.len() {
            return false;
        }
        self.items.remove(self.active + 1);
        true
    }

    fn build_content(&self, focused: bool) -> (Vec<Span>, usize) {
        let mut spans = Vec::<Span>::new();
        let mut width = 0usize;
        let mut cursor_width = 0usize;

        spans.push(Span::new("[").no_wrap());
        width += 1;

        for (idx, item) in self.items.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::new(", ").no_wrap());
                width += 2;
            }

            let display = if item.is_empty() {
                " ".to_string()
            } else {
                item.clone()
            };
            let is_active = focused && idx == self.active;
            let style = if is_active {
                Style::new().color(Color::Cyan).bold()
            } else {
                Style::default()
            };
            let display_width = UnicodeWidthStr::width(display.as_str());
            if is_active {
                let cursor_chars = self.cursor.min(text_edit::char_count(item.as_str()));
                cursor_width = width + width_of_char_prefix(item.as_str(), cursor_chars);
            }
            spans.push(Span::styled(display, style).no_wrap());
            width += display_width;
        }

        spans.push(Span::new("]").no_wrap());

        (spans, cursor_width)
    }
}

impl Drawable for ArrayInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let (content, _) = self.build_content(focused);
        let spans = content;
        DrawOutput::with_lines(vec![spans])
    }
}

impl Interactive for ArrayInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn store_sync_policy(&self) -> StoreSyncPolicy {
        StoreSyncPolicy::PreserveLocalStateWhileFocused
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(',') | KeyCode::Char(';') => {
                self.split_active();
                InteractionResult::handled()
            }
            _ => {
                let mut cursor = self.cursor;
                let outcome = {
                    let item = self.active_item_mut();
                    text_edit::apply_single_line_key(item, &mut cursor, key)
                };
                self.cursor = cursor;
                match outcome {
                    text_edit::TextKeyOutcome::Ignored => InteractionResult::ignored(),
                    text_edit::TextKeyOutcome::Changed | text_edit::TextKeyOutcome::CursorMoved => {
                        InteractionResult::handled()
                    }
                    text_edit::TextKeyOutcome::Submit => InteractionResult::input_done(),
                    text_edit::TextKeyOutcome::BackspaceAtStart => {
                        InteractionResult::handled_if(self.backspace_at_start())
                    }
                    text_edit::TextKeyOutcome::DeleteAtEnd => {
                        InteractionResult::handled_if(self.delete_at_end())
                    }
                    text_edit::TextKeyOutcome::MoveLeftAtStart => {
                        InteractionResult::handled_if(self.move_left_at_start())
                    }
                    text_edit::TextKeyOutcome::MoveRightAtEnd => {
                        InteractionResult::handled_if(self.move_right_at_end())
                    }
                }
            }
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let changed = match action {
            TextAction::DeleteWordLeft => self.remove_active(),
            TextAction::DeleteWordRight => self.remove_next(),
            TextAction::MoveWordLeft | TextAction::MoveWordRight => false,
        };
        if changed {
            self.normalize_items();
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::List(
            self.items
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .map(Value::Text)
                .collect::<Vec<_>>(),
        ))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(items) = value.to_text_list() {
            if Self::normalized_items(items.clone()) == Self::normalized_items(self.items.clone()) {
                return;
            }
            self.replace_items_preserving_state(items);
            return;
        }
        if let Some(text) = value.as_text() {
            let parts = text
                .split(&[',', ';'][..])
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            if Self::normalized_items(parts.clone()) == Self::normalized_items(self.items.clone()) {
                return;
            }
            self.replace_items_preserving_state(parts);
            return;
        }
        if matches!(value, Value::None) {
            if Self::normalized_items(self.items.clone()).is_empty() {
                return;
            }
            self.replace_items_preserving_state(Vec::new());
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        for item in &self.items {
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                run_validators(&self.validators, &Value::Text(trimmed.to_string()))?;
            }
        }
        Ok(())
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let (_, cursor_offset) = self.build_content(true);
        Some(CursorPos {
            col: cursor_offset as u16,
            row: 0,
        })
    }
}

fn width_of_char_prefix(value: &str, chars: usize) -> usize {
    value
        .chars()
        .take(chars)
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::ArrayInput;
    use crate::config::load_from_yaml_str;
    use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
    use crate::ui::render_view::RenderView;
    use crate::ui::renderer::{Renderer, RendererConfig};
    use crate::widgets::traits::Interactive;

    #[test]
    fn cursor_stays_after_inserted_char_in_first_item() {
        let mut input =
            ArrayInput::new("tags", "Tags").with_items(vec!["rust".into(), "tui".into()]);

        input.on_key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
        });
        input.on_key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
        });

        let cursor = input.cursor_pos().expect("cursor");
        assert_eq!(cursor.col, 6);
    }

    #[test]
    fn rendered_frame_cursor_matches_array_input_offset() {
        let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: array_input
        id: tags
        label: Tags
        items: [rust, tui]
"#;

        let loaded = load_from_yaml_str(yaml).expect("load config");
        let mut state = loaded.into_app_state().expect("app state");
        assert_eq!(state.focused_id(), Some("tags"));

        state.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
        });
        state.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
        });

        let view = RenderView::from_state(&state);
        let mut renderer = Renderer::new(RendererConfig {
            chrome_enabled: false,
        });
        let frame = renderer.render(
            &view,
            crate::terminal::TerminalSize {
                width: 80,
                height: 20,
            },
        );

        assert_eq!(frame.cursor.expect("frame cursor").col, 12);
    }

    #[test]
    fn bound_array_input_keeps_cursor_after_store_sync() {
        let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: array_input
        id: tags
        label: Tags
        items: [rust, tui]
        value: profile.tags
"#;

        let loaded = load_from_yaml_str(yaml).expect("load config");
        let mut state = loaded.into_app_state().expect("app state");
        assert_eq!(state.focused_id(), Some("tags"));

        state.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
        });
        state.dispatch_key_to_focused(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
        });

        let view = RenderView::from_state(&state);
        let mut renderer = Renderer::new(RendererConfig {
            chrome_enabled: false,
        });
        let frame = renderer.render(
            &view,
            crate::terminal::TerminalSize {
                width: 80,
                height: 20,
            },
        );

        assert_eq!(frame.cursor.expect("frame cursor").col, 12);
    }
}
