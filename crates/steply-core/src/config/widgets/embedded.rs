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

type EmbeddedNodeFactory = Arc<dyn Fn(String, String) -> Box<dyn InteractiveNode> + Send + Sync>;

pub(in crate::config) struct EmbeddedWidgetRegistryEntry {
    pub(in crate::config) doc: WidgetDocDescriptor,
    pub(in crate::config) build_doc: fn(WidgetDocDescriptor) -> Result<WidgetDoc, String>,
    pub(in crate::config) validate: fn(&EmbeddedWidgetDef) -> Result<(), String>,
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
    },
];

pub(super) fn embedded_widget_registry() -> &'static [EmbeddedWidgetRegistryEntry] {
    EMBEDDED_WIDGET_REGISTRY
}

pub(super) fn compile_table_embedded_factory(
    def: EmbeddedWidgetDef,
) -> Result<CellFactory, String> {
    compile_embedded_factory(def)
}

fn validate_embedded_widget_def(def: &EmbeddedWidgetDef) -> Result<(), String> {
    let widget_type = embedded_widget_type(def);
    let Some(entry) = embedded_widget_entry(widget_type) else {
        return Err(format!(
            "internal embedded widget registry is missing entry for '{widget_type}'"
        ));
    };
    (entry.validate)(def)
}

fn compile_embedded_factory(def: EmbeddedWidgetDef) -> Result<EmbeddedNodeFactory, String> {
    validate_embedded_widget_def(&def)?;
    match def {
        EmbeddedWidgetDef::TextInput(model::EmbeddedTextInputDef { placeholder, mode }) => {
            let mode = parse_text_mode(mode.as_deref())?;
            Ok(Arc::new(move |id, label| {
                let mut input = TextInput::new(id, label).with_mode(mode);
                if let Some(placeholder) = &placeholder {
                    input = input.with_placeholder(placeholder.clone());
                }
                Box::new(input)
            }))
        }
        EmbeddedWidgetDef::MaskedInput(model::EmbeddedMaskedInputDef { mask }) => {
            Ok(Arc::new(move |id, label| {
                Box::new(MaskedInput::new(id, label, mask.clone()))
            }))
        }
        EmbeddedWidgetDef::Select(model::EmbeddedSelectDef { options }) => {
            Ok(Arc::new(move |id, label| {
                Box::new(SelectInput::new(id, label, options.clone()))
            }))
        }
        EmbeddedWidgetDef::Slider(model::EmbeddedSliderDef {
            min,
            max,
            step,
            unit,
        }) => Ok(Arc::new(move |id, label| {
            let mut input = SliderInput::new(id, label, min, max);
            if let Some(step) = step {
                input = input.with_step(step);
            }
            if let Some(unit) = &unit {
                input = input.with_unit(unit.clone());
            }
            Box::new(input)
        })),
        EmbeddedWidgetDef::Checkbox(model::EmbeddedCheckboxDef { checked }) => {
            Ok(Arc::new(move |id, label| {
                let mut input = CheckboxInput::new(id, label);
                if checked.unwrap_or(false) {
                    input = input.with_checked(true);
                }
                Box::new(input)
            }))
        }
        EmbeddedWidgetDef::ArrayInput(model::EmbeddedArrayInputDef { items }) => {
            Ok(Arc::new(move |id, label| {
                Box::new(ArrayInput::new(id, label).with_items(items.clone()))
            }))
        }
    }
}

fn embedded_widget_entry(widget_type: &str) -> Option<&'static EmbeddedWidgetRegistryEntry> {
    embedded_widget_registry()
        .iter()
        .find(|entry| entry.doc.widget_type == widget_type)
}

fn embedded_widget_type(def: &EmbeddedWidgetDef) -> &'static str {
    match def {
        EmbeddedWidgetDef::TextInput(_) => "text_input",
        EmbeddedWidgetDef::MaskedInput(_) => "masked_input",
        EmbeddedWidgetDef::Select(_) => "select",
        EmbeddedWidgetDef::Slider(_) => "slider",
        EmbeddedWidgetDef::Checkbox(_) => "checkbox",
        EmbeddedWidgetDef::ArrayInput(_) => "array_input",
    }
}

fn embedded_registry_dispatch_mismatch<T>(widget_type: &str) -> Result<T, String> {
    Err(format!(
        "internal embedded widget registry dispatch mismatch for '{widget_type}'"
    ))
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
        _ => embedded_registry_dispatch_mismatch("text_input"),
    }
}
