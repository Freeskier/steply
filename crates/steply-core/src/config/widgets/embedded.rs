use std::sync::Arc;

use crate::widgets::components::table::CellFactory;
use crate::widgets::{
    components::repeater::RepeaterFieldFactory,
    inputs::{
        array::ArrayInput, checkbox::CheckboxInput, masked::MaskedInput, select::SelectInput,
        slider::SliderInput, text::TextInput,
    },
    traits::InteractiveNode,
};

use super::super::model::EmbeddedWidgetDef;
use super::super::parse::parse_text_mode;

pub(super) fn compile_table_embedded_factory(
    def: EmbeddedWidgetDef,
) -> Result<CellFactory, String> {
    let template = def.clone();
    validate_embedded_widget_def(&template)?;
    Ok(Arc::new(move |id, label| {
        compile_embedded_widget(template.clone(), id, label)
            .expect("embedded widget template validated")
    }))
}

pub(super) fn compile_repeater_embedded_factory(
    def: EmbeddedWidgetDef,
) -> Result<RepeaterFieldFactory, String> {
    let template = def.clone();
    validate_embedded_widget_def(&template)?;
    Ok(Arc::new(move |id, label| {
        compile_embedded_widget(template.clone(), id, label)
            .expect("embedded widget template validated")
    }))
}

fn validate_embedded_widget_def(def: &EmbeddedWidgetDef) -> Result<(), String> {
    if let EmbeddedWidgetDef::TextInput(super::super::model::EmbeddedTextInputDef {
        mode: Some(mode),
        ..
    }) = def
    {
        let _ = parse_text_mode(Some(mode.as_str()))?;
    }
    Ok(())
}

fn compile_embedded_widget(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::TextInput(super::super::model::EmbeddedTextInputDef {
            placeholder,
            mode,
        }) => {
            let mut input = TextInput::new(id, label).with_mode(parse_text_mode(mode.as_deref())?);
            if let Some(placeholder) = placeholder {
                input = input.with_placeholder(placeholder);
            }
            Ok(Box::new(input))
        }
        EmbeddedWidgetDef::MaskedInput(super::super::model::EmbeddedMaskedInputDef { mask }) => {
            Ok(Box::new(MaskedInput::new(id, label, mask)))
        }
        EmbeddedWidgetDef::Select(super::super::model::EmbeddedSelectDef { options }) => {
            Ok(Box::new(SelectInput::new(id, label, options)))
        }
        EmbeddedWidgetDef::Slider(super::super::model::EmbeddedSliderDef {
            min,
            max,
            step,
            unit,
        }) => {
            let mut input = SliderInput::new(id, label, min, max);
            if let Some(step) = step {
                input = input.with_step(step);
            }
            if let Some(unit) = unit {
                input = input.with_unit(unit);
            }
            Ok(Box::new(input))
        }
        EmbeddedWidgetDef::Checkbox(super::super::model::EmbeddedCheckboxDef { checked }) => {
            let mut input = CheckboxInput::new(id, label);
            if checked.unwrap_or(false) {
                input = input.with_checked(true);
            }
            Ok(Box::new(input))
        }
        EmbeddedWidgetDef::ArrayInput(super::super::model::EmbeddedArrayInputDef { items }) => {
            Ok(Box::new(ArrayInput::new(id, label).with_items(items)))
        }
    }
}
