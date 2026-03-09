use crate::widgets::components::calendar::Calendar;
use crate::widgets::components::file_browser::FileBrowserInput;
use crate::widgets::components::textarea::TextAreaComponent;
use crate::widgets::inputs::array::ArrayInput;
use crate::widgets::inputs::button::ButtonInput;
use crate::widgets::inputs::checkbox::CheckboxInput;
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::color::ColorInput;
use crate::widgets::inputs::masked::MaskedInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::slider::SliderInput;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::validators;

use super::super::model::ValidatorDef;
use super::super::parse::{
    WithChangeTargetPathValue, WithSubmitTargetPathValue, compile_validators, parse_value_target,
};

pub(super) trait SupportsValidator: Sized {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self;
}

impl SupportsValidator for TextInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for ArrayInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for ButtonInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for SelectInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for ChoiceInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for MaskedInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for SliderInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for ColorInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for CheckboxInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for Calendar {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for TextAreaComponent {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

impl SupportsValidator for FileBrowserInput {
    fn with_runtime_validator(self, validator: validators::Validator) -> Self {
        self.with_validator(validator)
    }
}

pub(super) fn with_required_and_validators<T>(
    mut widget: T,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> T
where
    T: SupportsValidator,
{
    if required.unwrap_or(false) {
        widget = widget.with_runtime_validator(validators::required_msg("Field is required"));
    }
    for validator in compile_validators(extra_validators) {
        widget = widget.with_runtime_validator(validator);
    }
    widget
}

pub(super) fn with_submit_target<T>(widget: T, submit_target: Option<String>) -> Result<T, String>
where
    T: WithSubmitTargetPathValue,
{
    match submit_target {
        Some(target) => {
            Ok(widget.with_submit_target_path_value(parse_value_target(target.as_str())?))
        }
        None => Ok(widget),
    }
}

pub(super) fn with_change_targets<T>(
    mut widget: T,
    change_targets: Vec<String>,
) -> Result<T, String>
where
    T: WithChangeTargetPathValue,
{
    for target in change_targets {
        widget = widget.with_change_target_path_value(parse_value_target(target.as_str())?);
    }
    Ok(widget)
}
