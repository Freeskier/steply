use crate::runtime::event::WidgetAction;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::traits::{InteractionResult, Interactive, TextAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEscBehavior {
    Hide,
    Blur,
}

#[derive(Debug, Clone)]
pub enum FilterKeyOutcome {
    Ignored,
    Hide,
    Blur,
    Edited(InteractionResult),
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

    match key.code {
        KeyCode::Esc => match esc_behavior {
            FilterEscBehavior::Hide => FilterKeyOutcome::Hide,
            FilterEscBehavior::Blur => FilterKeyOutcome::Blur,
        },
        KeyCode::Enter | KeyCode::Down => FilterKeyOutcome::Blur,
        _ => FilterKeyOutcome::Edited(sanitize_interaction_result(filter.on_key(key))),
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
