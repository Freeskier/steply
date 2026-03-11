use crate::core::value::Value;
use crate::widgets::{
    inputs::{
        array::ArrayInput, button::ButtonInput, checkbox::CheckboxInput, choice::ChoiceInput,
        color::ColorInput, confirm::ConfirmInput, masked::MaskedInput, select::SelectInput,
        slider::SliderInput, text::TextInput,
    },
    node::Node,
    validators,
};

use super::super::model::{ConfirmModeDef, ValidatorDef};
use super::super::parse::{compile_validators, parse_confirm_mode, parse_text_mode};
use super::common::with_required_and_validators;

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_text_input(
    id: String,
    label: String,
    placeholder: Option<String>,
    default: Option<String>,
    mode: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
    completion_items: Vec<String>,
) -> Result<Node, String> {
    let mut input = TextInput::new(id, label)
        .with_mode(parse_text_mode(mode.as_deref())?)
        .with_completion_items(completion_items);
    if let Some(placeholder) = placeholder {
        input = input.with_placeholder(placeholder);
    }
    if let Some(default) = default {
        input = input.with_default(Value::Text(default));
    }
    input = with_required_and_validators(input, required, extra_validators);
    Ok(Node::Input(Box::new(input)))
}

pub(super) fn compile_array_input(
    id: String,
    label: String,
    items: Vec<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let input = with_required_and_validators(
        ArrayInput::new(id, label).with_items(items),
        required,
        extra_validators,
    );
    Ok(Node::Input(Box::new(input)))
}

pub(super) fn compile_button_input(
    id: String,
    label: String,
    text: Option<String>,
    task_id: Option<String>,
) -> Result<Node, String> {
    let mut input = ButtonInput::new(id, label);
    if let Some(text) = text {
        input = input.with_text(text);
    }
    if let Some(task_id) = task_id {
        input = input.with_task_id(task_id);
    }
    Ok(Node::Input(Box::new(input)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_select_input(
    id: String,
    label: String,
    options: Vec<String>,
    selected: Option<usize>,
    default: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut input = SelectInput::new(id, label, options);
    if let Some(selected) = selected {
        input = input.with_selected(selected);
    }
    if let Some(default) = default {
        input = input.with_default(Value::Text(default));
    }
    input = with_required_and_validators(input, required, extra_validators);
    Ok(Node::Input(Box::new(input)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_choice_input(
    id: String,
    label: String,
    options: Vec<String>,
    bullets: Option<bool>,
    default: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut input = ChoiceInput::new(id, label, options).with_bullets(bullets.unwrap_or(true));
    if let Some(default) = default {
        input = input.with_default(Value::Text(default));
    }
    input = with_required_and_validators(input, required, extra_validators);
    Ok(Node::Input(Box::new(input)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_masked_input(
    id: String,
    label: String,
    mask: String,
    default: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut input = MaskedInput::new(id, label, mask);
    if let Some(default) = default {
        input = input.with_default(Value::Text(default));
    }
    input = with_required_and_validators(input, required, extra_validators);
    Ok(Node::Input(Box::new(input)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_slider_input(
    id: String,
    label: String,
    min: i64,
    max: i64,
    step: Option<i64>,
    unit: Option<String>,
    track_len: Option<usize>,
    default: Option<f64>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut widget = SliderInput::new(id, label, min, max);
    if let Some(step) = step {
        widget = widget.with_step(step);
    }
    if let Some(unit) = unit {
        widget = widget.with_unit(unit);
    }
    if let Some(track_len) = track_len {
        widget = widget.with_track_len(track_len);
    }
    if let Some(default) = default {
        widget = widget.with_default(Value::Number(default));
    }
    widget = with_required_and_validators(widget, required, extra_validators);
    Ok(Node::Input(Box::new(widget)))
}

pub(super) fn compile_color_input(
    id: String,
    label: String,
    rgb: Option<[u8; 3]>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut widget = ColorInput::new(id, label);
    if let Some([r, g, b]) = rgb {
        widget = widget.with_rgb(r, g, b);
    }
    widget = with_required_and_validators(widget, required, extra_validators);
    Ok(Node::Input(Box::new(widget)))
}

pub(super) fn compile_confirm_input(
    id: String,
    label: String,
    mode: Option<ConfirmModeDef>,
    yes_label: Option<String>,
    no_label: Option<String>,
    default: Option<bool>,
) -> Result<Node, String> {
    let mut widget = ConfirmInput::new(id, label).with_mode(parse_confirm_mode(mode));
    if let (Some(yes), Some(no)) = (yes_label, no_label) {
        widget = widget.with_options(yes, no);
    }
    if let Some(default) = default {
        widget = widget.with_default(Value::Bool(default));
    }
    Ok(Node::Input(Box::new(widget)))
}

pub(super) fn compile_checkbox_input(
    id: String,
    label: String,
    checked: Option<bool>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut widget = CheckboxInput::new(id, label);
    if checked.unwrap_or(false) {
        widget = widget.with_checked(true);
    }
    if required.unwrap_or(false) {
        widget = widget.with_validator(validators::must_be_checked());
    }
    for validator in compile_validators(extra_validators) {
        widget = widget.with_validator(validator);
    }
    Ok(Node::Input(Box::new(widget)))
}
