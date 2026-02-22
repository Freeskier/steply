use super::text_edit;
use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::{ValueChange, WidgetAction};
use crate::terminal::{CursorPos, KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextAction, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};
use unicode_width::UnicodeWidthChar;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextMode {

    #[default]
    Plain,



    Password,


    Secret,
}

pub struct TextInput {
    base: WidgetBase,
    value: String,
    cursor: usize,
    mode: TextMode,
    placeholder: Option<String>,
    submit_target: Option<ValueTarget>,
    change_target: Option<ValueTarget>,
    validators: Vec<Validator>,
    completion_items: Vec<String>,
}

impl TextInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            value: String::new(),
            cursor: 0,
            mode: TextMode::Plain,
            placeholder: None,
            submit_target: None,
            change_target: None,
            validators: Vec::new(),
            completion_items: Vec::new(),
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn with_default(mut self, value: impl Into<crate::core::value::Value>) -> Self {
        self.set_value(value.into());
        self
    }

    pub fn with_mode(mut self, mode: TextMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn with_change_target(mut self, target: impl Into<NodeId>) -> Self {
        self.change_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_change_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.change_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_completion_items(mut self, items: Vec<String>) -> Self {
        self.completion_items = items;
        self
    }

    pub fn set_completion_items(&mut self, items: Vec<String>) {
        self.completion_items = items;
    }

    pub fn completion_items_mut(&mut self) -> &mut Vec<String> {
        &mut self.completion_items
    }

    fn display_value(&self) -> String {
        let len = text_edit::char_count(&self.value);
        match self.mode {
            TextMode::Plain => self.value.clone(),
            TextMode::Password => "*".repeat(len),
            TextMode::Secret => " ".repeat(len),
        }
    }

    fn edited_result(&self) -> InteractionResult {
        if let Some(target) = &self.change_target {
            return InteractionResult::with_action(WidgetAction::ValueChanged {
                change: ValueChange::with_target(target.clone(), Value::Text(self.value.clone())),
            });
        }
        InteractionResult::handled()
    }
}

impl Drawable for TextInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);

        let ghost_suffix = if self.mode == TextMode::Plain && focused {
            ctx.completion_menus
                .get(self.base.id())
                .and_then(|menu| {
                    menu.matches
                        .get(menu.selected)
                        .map(|selected| (menu, selected))
                })
                .and_then(|(menu, selected)| {
                    completion_suffix(selected, &self.value, self.cursor, menu.start)
                })
                .filter(|suffix| !suffix.is_empty())
        } else {
            None
        };

        let mut first_line = if self.value.is_empty() && ghost_suffix.is_none() {
            if let Some(ph) = &self.placeholder {
                vec![Span::styled(ph.clone(), Style::new().color(Color::DarkGrey)).no_wrap()]
            } else {
                vec![Span::new(self.display_value()).no_wrap()]
            }
        } else {
            vec![Span::styled(self.display_value(), Style::default()).no_wrap()]
        };


        if let Some(suffix) = ghost_suffix {
            first_line.push(Span::styled(suffix, Style::new().color(Color::DarkGrey)).no_wrap());
        }

        DrawOutput {
            lines: vec![first_line],
        }
    }
}

impl Interactive for TextInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Char(ch) => {
                text_edit::insert_char(&mut self.value, &mut self.cursor, ch);
                self.edited_result()
            }
            KeyCode::Backspace => {
                if text_edit::backspace_char(&mut self.value, &mut self.cursor) {
                    return self.edited_result();
                }
                InteractionResult::ignored()
            }
            KeyCode::Delete => {
                if text_edit::delete_char(&mut self.value, &mut self.cursor) {
                    return self.edited_result();
                }
                InteractionResult::ignored()
            }

            KeyCode::Left if self.mode != TextMode::Secret => {
                if text_edit::move_left(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Right if self.mode != TextMode::Secret => {
                if text_edit::move_right(&mut self.cursor, &self.value) {
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }
            KeyCode::Home if self.mode != TextMode::Secret => {
                self.cursor = 0;
                InteractionResult::handled()
            }
            KeyCode::End if self.mode != TextMode::Secret => {
                self.cursor = text_edit::char_count(&self.value);
                InteractionResult::handled()
            }
            KeyCode::Enter => InteractionResult::submit_or_produce(
                self.submit_target.as_ref(),
                Value::Text(self.value.clone()),
            ),
            _ => InteractionResult::ignored(),
        }
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {


        if self.mode != TextMode::Plain {
            return None;
        }
        Some(TextEditState {
            value: &mut self.value,
            cursor: &mut self.cursor,
        })
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {

        if self.mode != TextMode::Plain {
            return None;
        }
        Some(CompletionState {
            value: &mut self.value,
            cursor: &mut self.cursor,
            candidates: self.completion_items.as_slice(),
            prefix_start: None,
        })
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        let Some(mut state) = self.text_editing() else {
            return InteractionResult::ignored();
        };
        if action.apply(&mut state) {
            self.on_text_edited();
            return self.edited_result();
        }
        InteractionResult::ignored()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.value.clone()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(v) = value.to_text_scalar() {
            self.value = v;
            self.cursor = text_edit::char_count(&self.value);
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &Value::Text(self.value.clone()))
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let col = match self.mode {
            TextMode::Secret => 0,
            TextMode::Password => text_edit::clamp_cursor(self.cursor, &self.value) as u16,
            TextMode::Plain => {
                let value_width: usize = self
                    .value
                    .chars()
                    .take(text_edit::clamp_cursor(self.cursor, &self.value))
                    .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
                    .sum();
                value_width as u16
            }
        };
        Some(CursorPos { col, row: 0 })
    }
}

fn completion_suffix(selected: &str, value: &str, cursor: usize, start: usize) -> Option<String> {
    let chars: Vec<char> = value.chars().collect();
    let pos = cursor.min(chars.len());
    let start = start.min(pos);
    let token: String = chars[start..pos].iter().collect();
    if !token.is_empty() && !selected.to_lowercase().starts_with(&token.to_lowercase()) {
        return None;
    }
    let token_len = token.chars().count();
    Some(selected.chars().skip(token_len).collect())
}
