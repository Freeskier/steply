use std::sync::Arc;

use crate::widgets::components::table::CellFactory;
use crate::widgets::inputs::{
    array::ArrayInput, checkbox::CheckboxInput, masked::MaskedInput, select::SelectInput,
    slider::SliderInput, text::TextInput,
};
use crate::widgets::traits::InteractiveNode;

use super::super::doc_model::{WidgetCategory, WidgetDoc, WidgetDocDescriptor, build_widget_doc};
use super::super::model::{self, EmbeddedWidgetDef};
use super::super::parse::parse_text_mode;
use crate::widgets::components::repeater::RepeaterFieldFactory;

type EmbeddedCompileFn =
    fn(EmbeddedWidgetDef, String, String) -> Result<Box<dyn InteractiveNode>, String>;

pub(in crate::config) struct EmbeddedWidgetRegistryEntry {
    pub(in crate::config) doc: WidgetDocDescriptor,
    pub(in crate::config) build_doc: fn(WidgetDocDescriptor) -> Result<WidgetDoc, String>,
    pub(in crate::config) validate: fn(&EmbeddedWidgetDef) -> Result<(), String>,
    pub(in crate::config) compile: EmbeddedCompileFn,
}

const fn embedded_doc(
    widget_type: &'static str,
    short_description: &'static str,
    long_description: &'static str,
    example_yaml: &'static str,
) -> WidgetDocDescriptor {
    WidgetDocDescriptor {
        widget_type,
        category: WidgetCategory::Embedded,
        short_description,
        long_description,
        example_yaml,
        static_hints: &[],
    }
}

const EMBEDDED_WIDGET_REGISTRY: &[EmbeddedWidgetRegistryEntry] = &[
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "text_input",
            "Embedded text input.",
            "Single-line text input usable inside tables and repeaters.",
            r#"type: text_input
placeholder: service-name"#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedTextInputDef>,
        validate: validate_embedded_text_input,
        compile: compile_embedded_text_input,
    },
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "masked_input",
            "Embedded masked input.",
            "Masked text input usable inside tables and repeaters.",
            r#"type: masked_input
mask: \"999-AAA\""#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedMaskedInputDef>,
        validate: validate_embedded_noop,
        compile: compile_embedded_masked_input,
    },
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "select",
            "Embedded select input.",
            "Embedded single-choice select field.",
            r#"type: select
options: [small, medium, large]"#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedSelectDef>,
        validate: validate_embedded_noop,
        compile: compile_embedded_select,
    },
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "slider",
            "Embedded slider input.",
            "Embedded numeric slider field.",
            r#"type: slider
min: 0
max: 100"#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedSliderDef>,
        validate: validate_embedded_noop,
        compile: compile_embedded_slider,
    },
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "checkbox",
            "Embedded checkbox input.",
            "Embedded boolean checkbox field.",
            r#"type: checkbox
checked: true"#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedCheckboxDef>,
        validate: validate_embedded_noop,
        compile: compile_embedded_checkbox,
    },
    EmbeddedWidgetRegistryEntry {
        doc: embedded_doc(
            "array_input",
            "Embedded array input.",
            "Embedded list input field.",
            r#"type: array_input
items: [a, b]"#,
        ),
        build_doc: build_widget_doc::<model::EmbeddedArrayInputDef>,
        validate: validate_embedded_noop,
        compile: compile_embedded_array_input,
    },
];

pub(super) fn embedded_widget_registry() -> &'static [EmbeddedWidgetRegistryEntry] {
    EMBEDDED_WIDGET_REGISTRY
}

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
    (embedded_widget_entry(def).validate)(def)
}

fn compile_embedded_widget(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    (embedded_widget_entry(&def).compile)(def, id, label)
}

fn embedded_widget_entry(def: &EmbeddedWidgetDef) -> &'static EmbeddedWidgetRegistryEntry {
    match def {
        EmbeddedWidgetDef::TextInput(_) => &EMBEDDED_WIDGET_REGISTRY[0],
        EmbeddedWidgetDef::MaskedInput(_) => &EMBEDDED_WIDGET_REGISTRY[1],
        EmbeddedWidgetDef::Select(_) => &EMBEDDED_WIDGET_REGISTRY[2],
        EmbeddedWidgetDef::Slider(_) => &EMBEDDED_WIDGET_REGISTRY[3],
        EmbeddedWidgetDef::Checkbox(_) => &EMBEDDED_WIDGET_REGISTRY[4],
        EmbeddedWidgetDef::ArrayInput(_) => &EMBEDDED_WIDGET_REGISTRY[5],
    }
}

fn validate_embedded_noop(_def: &EmbeddedWidgetDef) -> Result<(), String> {
    Ok(())
}

fn validate_embedded_text_input(def: &EmbeddedWidgetDef) -> Result<(), String> {
    match def {
        EmbeddedWidgetDef::TextInput(model::EmbeddedTextInputDef {
            mode: Some(mode), ..
        }) => {
            let _ = parse_text_mode(Some(mode.as_str()))?;
            Ok(())
        }
        EmbeddedWidgetDef::TextInput(_) => Ok(()),
        _ => unreachable!("embedded widget registry dispatch mismatch: text_input"),
    }
}

fn compile_embedded_text_input(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::TextInput(model::EmbeddedTextInputDef { placeholder, mode }) => {
            let mut input = TextInput::new(id, label).with_mode(parse_text_mode(mode.as_deref())?);
            if let Some(placeholder) = placeholder {
                input = input.with_placeholder(placeholder);
            }
            Ok(Box::new(input))
        }
        _ => unreachable!("embedded widget registry dispatch mismatch: text_input"),
    }
}

fn compile_embedded_masked_input(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::MaskedInput(model::EmbeddedMaskedInputDef { mask }) => {
            Ok(Box::new(MaskedInput::new(id, label, mask)))
        }
        _ => unreachable!("embedded widget registry dispatch mismatch: masked_input"),
    }
}

fn compile_embedded_select(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::Select(model::EmbeddedSelectDef { options }) => {
            Ok(Box::new(SelectInput::new(id, label, options)))
        }
        _ => unreachable!("embedded widget registry dispatch mismatch: select"),
    }
}

fn compile_embedded_slider(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::Slider(model::EmbeddedSliderDef {
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
        _ => unreachable!("embedded widget registry dispatch mismatch: slider"),
    }
}

fn compile_embedded_checkbox(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::Checkbox(model::EmbeddedCheckboxDef { checked }) => {
            let mut input = CheckboxInput::new(id, label);
            if checked.unwrap_or(false) {
                input = input.with_checked(true);
            }
            Ok(Box::new(input))
        }
        _ => unreachable!("embedded widget registry dispatch mismatch: checkbox"),
    }
}

fn compile_embedded_array_input(
    def: EmbeddedWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        EmbeddedWidgetDef::ArrayInput(model::EmbeddedArrayInputDef { items }) => {
            Ok(Box::new(ArrayInput::new(id, label).with_items(items)))
        }
        _ => unreachable!("embedded widget registry dispatch mismatch: array_input"),
    }
}
