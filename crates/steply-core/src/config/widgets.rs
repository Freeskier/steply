mod common;
mod components;
mod embedded;
mod inputs;
mod outputs;

use crate::core::store_refs::{
    exact_template_expr, normalize_store_selector, template_expressions,
};
use crate::widgets::node::Node;
use crate::widgets::shared::binding::{
    ReadBinding, StoreBinding, WriteBinding, WriteExpr, bind_node,
};
use crate::widgets::static_hints;

use super::binding_compile::{compile_read_binding_value, compile_write_bindings, parse_selector};
use super::doc_model::{WidgetCategory, WidgetDoc, WidgetDocDescriptor, build_widget_doc};
use super::model::{self, WidgetDef};

pub(super) struct WidgetRegistryEntry {
    pub(super) doc: WidgetDocDescriptor,
    pub(super) build_doc: fn(WidgetDocDescriptor) -> Result<WidgetDoc, String>,
    pub(super) compile: fn(WidgetDef) -> Result<Node, String>,
}

const fn widget_doc(
    widget_type: &'static str,
    category: WidgetCategory,
    short_description: &'static str,
    long_description: &'static str,
    example_yaml: &'static str,
    static_hints: &'static [crate::widgets::traits::StaticHintSpec],
) -> WidgetDocDescriptor {
    WidgetDocDescriptor {
        widget_type,
        category,
        short_description,
        long_description,
        example_yaml,
        static_hints,
    }
}

const WIDGET_REGISTRY: &[WidgetRegistryEntry] = &[
    WidgetRegistryEntry {
        doc: widget_doc(
            "text_output",
            WidgetCategory::Output,
            "Static text output.",
            "Renders a fixed text block in the flow.",
            r#"type: text_output
id: intro
text: Welcome to Steply"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::TextOutputDef>,
        compile: compile_text_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "url_output",
            WidgetCategory::Output,
            "Clickable URL output.",
            "Displays a URL with an optional human-friendly label.",
            r#"type: url_output
id: docs
url: https://example.com
name: Open docs"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::UrlOutputDef>,
        compile: compile_url_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "thinking_output",
            WidgetCategory::Output,
            "Animated thinking output.",
            "Shows animated status text for background work or waiting states.",
            r#"type: thinking_output
id: thinking
label: Preparing
text: Resolving inputs"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ThinkingOutputDef>,
        compile: compile_thinking_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "progress_output",
            WidgetCategory::Output,
            "Progress bar output.",
            "Displays progress with optional range, style and transition settings.",
            r#"type: progress_output
id: build_progress
label: Build
min: 0
max: 100"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ProgressOutputDef>,
        compile: compile_progress_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "chart_output",
            WidgetCategory::Output,
            "Terminal chart output.",
            "Renders numeric values as a small terminal chart.",
            r#"type: chart_output
id: cpu
label: CPU load"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ChartOutputDef>,
        compile: compile_chart_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "table_output",
            WidgetCategory::Output,
            "Read-only table output.",
            "Displays tabular data without inline editing.",
            r#"type: table_output
id: summary
label: Summary
headers: [Name, Value]"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::TableOutputDef>,
        compile: compile_table_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "diff_output",
            WidgetCategory::Output,
            "Text diff output.",
            "Shows the difference between two text values.",
            r#"type: diff_output
id: diff
label: Planned changes
old: before
new: after"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::DiffOutputDef>,
        compile: compile_diff_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "task_log_output",
            WidgetCategory::Output,
            "Task log output.",
            "Displays task execution progress as a step-by-step log.",
            r#"type: task_log_output
id: task_log
steps:
  - label: Clone repo
    task_id: clone"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::TaskLogOutputDef>,
        compile: compile_task_log_output_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "text_input",
            WidgetCategory::Input,
            "Single-line text input.",
            "Collects one line of text with optional validation and completion.",
            r#"type: text_input
id: project_name
label: Project name"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::TextInputDef>,
        compile: compile_text_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "array_input",
            WidgetCategory::Input,
            "Array input.",
            "Collects multiple string values as a list.",
            r#"type: array_input
id: tags
label: Tags"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ArrayInputDef>,
        compile: compile_array_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "button_input",
            WidgetCategory::Input,
            "Button input.",
            "Focusable button that may trigger a task when activated.",
            r#"type: button_input
id: refresh
label: Refresh"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ButtonInputDef>,
        compile: compile_button_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "select",
            WidgetCategory::Input,
            "Single-select input.",
            "Lets the user choose one option from a list.",
            r#"type: select
id: region
label: Region
options: [eu, us]"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::SelectDef>,
        compile: compile_select_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "choice_input",
            WidgetCategory::Input,
            "Choice input.",
            "Choice selector rendered as navigable options.",
            r#"type: choice_input
id: package_manager
label: Package manager
options: [cargo, npm]"#,
            static_hints::CHOICE_INPUT_HINTS,
        ),
        build_doc: build_widget_doc::<model::ChoiceInputDef>,
        compile: compile_choice_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "select_list",
            WidgetCategory::Component,
            "Selectable list component.",
            "Interactive list component supporting single or multi selection.",
            r#"type: select_list
id: features
label: Features
options: [auth, api]"#,
            static_hints::SELECT_LIST_DOC_HINTS,
        ),
        build_doc: build_widget_doc::<model::SelectListDef>,
        compile: compile_select_list_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "masked_input",
            WidgetCategory::Input,
            "Masked input.",
            "Text input constrained by a mask pattern.",
            r#"type: masked_input
id: phone
label: Phone
mask: \"(999) 999-9999\""#,
            &[],
        ),
        build_doc: build_widget_doc::<model::MaskedInputDef>,
        compile: compile_masked_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "slider",
            WidgetCategory::Input,
            "Slider input.",
            "Numeric slider with configurable range and step.",
            r#"type: slider
id: retries
label: Retries
min: 0
max: 10"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::SliderDef>,
        compile: compile_slider_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "color_input",
            WidgetCategory::Input,
            "Color input.",
            "Picks a color represented as RGB values.",
            r#"type: color_input
id: accent
label: Accent color"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::ColorInputDef>,
        compile: compile_color_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "confirm_input",
            WidgetCategory::Input,
            "Confirmation input.",
            "Confirms a yes/no choice in relaxed or strict mode.",
            r#"type: confirm_input
id: proceed
label: Continue?"#,
            static_hints::CONFIRM_STRICT_HINTS,
        ),
        build_doc: build_widget_doc::<model::ConfirmInputDef>,
        compile: compile_confirm_input_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "checkbox",
            WidgetCategory::Input,
            "Checkbox input.",
            "Single boolean checkbox control.",
            r#"type: checkbox
id: accept
label: Accept terms"#,
            &[],
        ),
        build_doc: build_widget_doc::<model::CheckboxDef>,
        compile: compile_checkbox_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "calendar",
            WidgetCategory::Component,
            "Calendar component.",
            "Interactive date, time or date-time picker.",
            r#"type: calendar
id: deploy_at
label: Deploy at"#,
            static_hints::CALENDAR_COMMON_HINTS,
        ),
        build_doc: build_widget_doc::<model::CalendarDef>,
        compile: compile_calendar_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "textarea",
            WidgetCategory::Component,
            "Textarea component.",
            "Multi-line text editor widget.",
            r#"type: textarea
id: notes
min_height: 4"#,
            static_hints::TEXTAREA_HINTS,
        ),
        build_doc: build_widget_doc::<model::TextareaDef>,
        compile: compile_textarea_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "command_runner",
            WidgetCategory::Component,
            "Command runner.",
            "Runs one or more shell commands inside the flow.",
            r#"type: command_runner
id: install
label: Install dependencies
commands:
  - label: Cargo fetch
    program: cargo
    args: [fetch]"#,
            static_hints::COMMAND_RUNNER_HINTS,
        ),
        build_doc: build_widget_doc::<model::CommandRunnerDef>,
        compile: compile_command_runner_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "file_browser",
            WidgetCategory::Component,
            "File browser component.",
            "Interactive file or directory browser with optional completion.",
            r#"type: file_browser
id: project_dir
label: Select project directory"#,
            static_hints::FILE_BROWSER_DOC_HINTS,
        ),
        build_doc: build_widget_doc::<model::FileBrowserDef>,
        compile: compile_file_browser_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "tree_view",
            WidgetCategory::Component,
            "Tree view component.",
            "Interactive hierarchical tree selector.",
            r#"type: tree_view
id: modules
label: Modules
nodes:
  - item: core
    depth: 0
    has_children: true"#,
            static_hints::TREE_VIEW_DOC_HINTS,
        ),
        build_doc: build_widget_doc::<model::TreeViewDef>,
        compile: compile_tree_view_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "object_editor",
            WidgetCategory::Component,
            "Object editor.",
            "Structured editor for object-like values.",
            r#"type: object_editor
id: payload
label: Payload"#,
            static_hints::OBJECT_EDITOR_DOC_HINTS,
        ),
        build_doc: build_widget_doc::<model::ObjectEditorDef>,
        compile: compile_object_editor_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "snippet",
            WidgetCategory::Component,
            "Snippet component.",
            "Builds a snippet from nested interactive inputs.",
            r#"type: snippet
id: export_cmd
label: Export command
template: \"export NAME={{name}}\""#,
            static_hints::SNIPPET_HINTS,
        ),
        build_doc: build_widget_doc::<model::SnippetDef>,
        compile: compile_snippet_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "table",
            WidgetCategory::Component,
            "Editable table component.",
            "Edits repeated rows using embedded widgets per column.",
            r#"type: table
id: envs
label: Environments
columns:
  - header: Name
    widget:
      type: text_input"#,
            static_hints::TABLE_DOC_HINTS,
        ),
        build_doc: build_widget_doc::<model::TableDef>,
        compile: compile_table_widget,
    },
    WidgetRegistryEntry {
        doc: widget_doc(
            "repeater",
            WidgetCategory::Component,
            "Repeater component.",
            "Edits repeated items using embedded widget fields.",
            r#"type: repeater
id: services
label: Services
fields:
  - key: name
    label: Name
    widget:
      type: text_input"#,
            static_hints::REPEATER_HINTS,
        ),
        build_doc: build_widget_doc::<model::RepeaterDef>,
        compile: compile_repeater_widget,
    },
];

pub(super) fn widget_registry() -> &'static [WidgetRegistryEntry] {
    WIDGET_REGISTRY
}

pub(super) fn embedded_widget_registry() -> &'static [embedded::EmbeddedWidgetRegistryEntry] {
    embedded::embedded_widget_registry()
}

pub(super) fn walk_widgets(
    widgets: &[WidgetDef],
    visitor: &mut impl FnMut(&WidgetDef) -> Result<(), String>,
) -> Result<(), String> {
    for widget in widgets {
        visitor(widget)?;
        if let Some(children) = widget_children(widget) {
            walk_widgets(children, visitor)?;
        }
    }
    Ok(())
}

pub(super) fn widget_id(widget: &WidgetDef) -> &str {
    match widget {
        WidgetDef::TextOutput(def) => def.id.as_str(),
        WidgetDef::UrlOutput(def) => def.id.as_str(),
        WidgetDef::ThinkingOutput(def) => def.id.as_str(),
        WidgetDef::ProgressOutput(def) => def.id.as_str(),
        WidgetDef::ChartOutput(def) => def.id.as_str(),
        WidgetDef::TableOutput(def) => def.id.as_str(),
        WidgetDef::DiffOutput(def) => def.id.as_str(),
        WidgetDef::TaskLogOutput(def) => def.id.as_str(),
        WidgetDef::TextInput(def) => def.id.as_str(),
        WidgetDef::ArrayInput(def) => def.id.as_str(),
        WidgetDef::ButtonInput(def) => def.id.as_str(),
        WidgetDef::Select(def) => def.id.as_str(),
        WidgetDef::ChoiceInput(def) => def.id.as_str(),
        WidgetDef::SelectList(def) => def.id.as_str(),
        WidgetDef::MaskedInput(def) => def.id.as_str(),
        WidgetDef::Slider(def) => def.id.as_str(),
        WidgetDef::ColorInput(def) => def.id.as_str(),
        WidgetDef::ConfirmInput(def) => def.id.as_str(),
        WidgetDef::Checkbox(def) => def.id.as_str(),
        WidgetDef::Calendar(def) => def.id.as_str(),
        WidgetDef::Textarea(def) => def.id.as_str(),
        WidgetDef::CommandRunner(def) => def.id.as_str(),
        WidgetDef::FileBrowser(def) => def.id.as_str(),
        WidgetDef::TreeView(def) => def.id.as_str(),
        WidgetDef::ObjectEditor(def) => def.id.as_str(),
        WidgetDef::Snippet(def) => def.id.as_str(),
        WidgetDef::Table(def) => def.id.as_str(),
        WidgetDef::Repeater(def) => def.id.as_str(),
    }
}

pub(super) fn visit_widget_inline_task_ids(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(String) -> Result<(), String>,
) -> Result<(), String> {
    match widget {
        WidgetDef::CommandRunner(def) => {
            for (index, _) in def.commands.iter().enumerate() {
                visitor(format!("{}::command::{index}", def.id))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn visit_widget_task_references(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    match widget {
        WidgetDef::TaskLogOutput(def) => {
            for step in &def.steps {
                visitor(step.task_id.as_str())?;
            }
            Ok(())
        }
        WidgetDef::ButtonInput(def) => match &def.task_id {
            Some(task_id) => visitor(task_id.as_str()),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

pub(super) fn visit_widget_binding_read_selectors(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    visit_widget_option_selectors(widget, visitor)?;
    let Some(binding) = widget_binding(widget) else {
        return Ok(());
    };
    let Some(reads) = &binding.reads else {
        return Ok(());
    };
    visit_read_binding_selectors(reads, true, visitor)
}

pub(super) fn visit_widget_binding_write_targets(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let Some(binding) = widget_binding(widget) else {
        return Ok(());
    };
    match &binding.writes {
        None => Ok(()),
        Some(model::WriteBindingDef::Selector(selector)) => {
            visitor(normalize_binding_selector(selector)?.as_str())
        }
        Some(model::WriteBindingDef::Map(entries)) => {
            for target in entries.keys() {
                visitor(normalize_binding_selector(target)?.as_str())?;
            }
            Ok(())
        }
    }
}

pub(super) fn visit_widget_binding_direct_value_targets(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let Some(binding) = widget_binding(widget) else {
        return Ok(());
    };

    if let Some(selector) = &binding.value {
        return visitor(normalize_binding_selector(selector)?.as_str());
    }

    let Some(read_selector) = binding_top_level_read_selector(binding) else {
        return Ok(());
    };

    match &binding.writes {
        Some(model::WriteBindingDef::Selector(target))
            if normalize_binding_selector(target)? == read_selector =>
        {
            visitor(read_selector.as_str())
        }
        Some(model::WriteBindingDef::Map(entries)) if entries.len() == 1 => {
            let Some((target, expr)) = entries.iter().next() else {
                return Ok(());
            };
            let normalized_target = normalize_binding_selector(target)?;
            if normalized_target == read_selector && write_expr_is_identity(&expr.0) {
                visitor(read_selector.as_str())
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

pub(super) fn compile_widget(def: WidgetDef) -> Result<Node, String> {
    let binding = compile_store_binding(&def)?;
    let widget_type = widget_type(&def);
    let Some(entry) = widget_entry(widget_type) else {
        return Err(format!(
            "internal widget registry is missing entry for '{widget_type}'"
        ));
    };
    Ok(bind_node((entry.compile)(def)?, binding))
}

fn widget_entry(widget_type: &str) -> Option<&'static WidgetRegistryEntry> {
    widget_registry()
        .iter()
        .find(|entry| entry.doc.widget_type == widget_type)
}

fn widget_type(widget: &WidgetDef) -> &'static str {
    match widget {
        WidgetDef::TextOutput(_) => "text_output",
        WidgetDef::UrlOutput(_) => "url_output",
        WidgetDef::ThinkingOutput(_) => "thinking_output",
        WidgetDef::ProgressOutput(_) => "progress_output",
        WidgetDef::ChartOutput(_) => "chart_output",
        WidgetDef::TableOutput(_) => "table_output",
        WidgetDef::DiffOutput(_) => "diff_output",
        WidgetDef::TaskLogOutput(_) => "task_log_output",
        WidgetDef::TextInput(_) => "text_input",
        WidgetDef::ArrayInput(_) => "array_input",
        WidgetDef::ButtonInput(_) => "button_input",
        WidgetDef::Select(_) => "select",
        WidgetDef::ChoiceInput(_) => "choice_input",
        WidgetDef::SelectList(_) => "select_list",
        WidgetDef::MaskedInput(_) => "masked_input",
        WidgetDef::Slider(_) => "slider",
        WidgetDef::ColorInput(_) => "color_input",
        WidgetDef::ConfirmInput(_) => "confirm_input",
        WidgetDef::Checkbox(_) => "checkbox",
        WidgetDef::Calendar(_) => "calendar",
        WidgetDef::Textarea(_) => "textarea",
        WidgetDef::CommandRunner(_) => "command_runner",
        WidgetDef::FileBrowser(_) => "file_browser",
        WidgetDef::TreeView(_) => "tree_view",
        WidgetDef::ObjectEditor(_) => "object_editor",
        WidgetDef::Snippet(_) => "snippet",
        WidgetDef::Table(_) => "table",
        WidgetDef::Repeater(_) => "repeater",
    }
}

fn registry_dispatch_mismatch<T>(widget_type: &str) -> Result<T, String> {
    Err(format!(
        "internal widget registry dispatch mismatch for '{widget_type}'"
    ))
}

fn widget_children(widget: &WidgetDef) -> Option<&[WidgetDef]> {
    match widget {
        WidgetDef::Snippet(def) => Some(def.inputs.as_slice()),
        WidgetDef::Repeater(def) => Some(def.widgets.as_slice()),
        _ => None,
    }
}

fn widget_binding(widget: &WidgetDef) -> Option<&model::WidgetBindingDef> {
    match widget {
        WidgetDef::TextInput(def) => Some(&def.binding),
        WidgetDef::ArrayInput(def) => Some(&def.binding),
        WidgetDef::Select(def) => Some(&def.binding),
        WidgetDef::ChoiceInput(def) => Some(&def.binding),
        WidgetDef::SelectList(def) => Some(&def.binding),
        WidgetDef::MaskedInput(def) => Some(&def.binding),
        WidgetDef::Slider(def) => Some(&def.binding),
        WidgetDef::ColorInput(def) => Some(&def.binding),
        WidgetDef::ConfirmInput(def) => Some(&def.binding),
        WidgetDef::Checkbox(def) => Some(&def.binding),
        WidgetDef::Calendar(def) => Some(&def.binding),
        WidgetDef::Textarea(def) => Some(&def.binding),
        WidgetDef::CommandRunner(def) => Some(&def.binding),
        WidgetDef::FileBrowser(def) => Some(&def.binding),
        WidgetDef::TreeView(def) => Some(&def.binding),
        WidgetDef::ObjectEditor(def) => Some(&def.binding),
        WidgetDef::Snippet(def) => Some(&def.binding),
        WidgetDef::Table(def) => Some(&def.binding),
        WidgetDef::Repeater(def) => Some(&def.binding),
        _ => None,
    }
}

fn visit_widget_option_selectors(
    widget: &WidgetDef,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    match widget {
        WidgetDef::Select(model::SelectDef {
            options: model::StringOptionsDef::Selector(selector),
            ..
        })
        | WidgetDef::ChoiceInput(model::ChoiceInputDef {
            options: model::StringOptionsDef::Selector(selector),
            ..
        })
        | WidgetDef::SelectList(model::SelectListDef {
            options: model::SelectListOptionsDef::Selector(selector),
            ..
        }) => visitor(normalize_binding_selector(selector)?.as_str()),
        _ => Ok(()),
    }
}

fn visit_read_binding_selectors(
    value: &serde_yaml::Value,
    top_level: bool,
    visitor: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    match value {
        serde_yaml::Value::String(text) => {
            if let Some(expr) = exact_template_expr(text)
                && let Ok(selector) = normalize_binding_selector(expr)
            {
                return visitor(selector.as_str());
            }
            if text.contains("{{") && text.contains("}}") {
                for expr in template_expressions(text) {
                    if let Ok(selector) = normalize_binding_selector(expr.as_str()) {
                        visitor(selector.as_str())?;
                    }
                }
                return Ok(());
            }
            if top_level && let Ok(selector) = normalize_binding_selector(text) {
                visitor(selector.as_str())?;
            }
            Ok(())
        }
        serde_yaml::Value::Mapping(map) => {
            for nested in map.values() {
                visit_read_binding_selectors(nested, false, visitor)?;
            }
            Ok(())
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                visit_read_binding_selectors(item, false, visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn compile_store_binding(def: &WidgetDef) -> Result<StoreBinding, String> {
    let mut binding = compile_widget_binding(def, widget_binding(def).cloned())?;
    binding.options = compile_option_binding(def)?;
    Ok(binding)
}

pub(super) fn compile_task_writes(
    writes: Option<model::WriteBindingDef>,
) -> Result<Vec<WriteBinding>, String> {
    compile_write_bindings(writes, "stdout", is_task_scope_ref)
}

fn compile_option_binding(def: &WidgetDef) -> Result<Option<ReadBinding>, String> {
    match def {
        WidgetDef::Select(model::SelectDef {
            options: model::StringOptionsDef::Selector(selector),
            ..
        })
        | WidgetDef::ChoiceInput(model::ChoiceInputDef {
            options: model::StringOptionsDef::Selector(selector),
            ..
        })
        | WidgetDef::SelectList(model::SelectListDef {
            options: model::SelectListOptionsDef::Selector(selector),
            ..
        }) => Ok(Some(ReadBinding::Selector(parse_selector(
            selector.as_str(),
        )?))),
        _ => Ok(None),
    }
}

fn compile_widget_binding(
    def: &WidgetDef,
    binding: Option<model::WidgetBindingDef>,
) -> Result<StoreBinding, String> {
    let Some(binding) = binding else {
        return Ok(StoreBinding::default());
    };

    if binding.value.is_some() && (binding.reads.is_some() || binding.writes.is_some()) {
        return Err("binding 'value' cannot be combined with 'reads' or 'writes'".to_string());
    }

    if let Some(selector) = binding.value {
        let target = parse_selector(selector.as_str())?;
        return Ok(StoreBinding {
            value: Some(target.clone()),
            options: None,
            reads: Some(ReadBinding::Selector(target.clone())),
            writes: vec![WriteBinding {
                target,
                expr: WriteExpr::ScopeRef("value".to_string()),
            }],
        });
    }

    let reads = binding
        .reads
        .map(|value| compile_read_binding_value(&value, true))
        .transpose()?;

    let writes = compile_write_bindings(binding.writes, "value", widget_scope_ref(def))?;

    Ok(StoreBinding {
        value: None,
        options: None,
        reads,
        writes,
    })
}

fn widget_scope_ref(def: &WidgetDef) -> fn(&str) -> bool {
    match def {
        WidgetDef::CommandRunner(_) => is_command_runner_scope_ref,
        _ => is_value_scope_ref,
    }
}

fn compile_string_options(options: model::StringOptionsDef) -> Vec<String> {
    match options {
        model::StringOptionsDef::Values(values) => values,
        model::StringOptionsDef::Selector(_) => Vec::new(),
    }
}

fn compile_select_list_options(
    options: model::SelectListOptionsDef,
) -> Vec<model::SelectListOptionDef> {
    match options {
        model::SelectListOptionsDef::Values(values) => values,
        model::SelectListOptionsDef::Selector(_) => Vec::new(),
    }
}

fn normalize_binding_selector(selector: &str) -> Result<String, String> {
    normalize_store_selector(selector)
}

fn is_value_scope_ref(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed == "value" || trimmed.starts_with("value.") || trimmed.starts_with("value[")
}

fn is_task_scope_ref(text: &str) -> bool {
    let trimmed = text.trim();
    matches!(
        trimmed,
        "stdout" | "stderr" | "exit_code" | "error" | "cancelled" | "task_id"
    ) || trimmed.starts_with("stdout.")
        || trimmed.starts_with("stdout[")
        || trimmed.starts_with("stderr.")
        || trimmed.starts_with("stderr[")
        || trimmed.starts_with("exit_code.")
        || trimmed.starts_with("exit_code[")
        || trimmed.starts_with("error.")
        || trimmed.starts_with("error[")
        || trimmed.starts_with("cancelled.")
        || trimmed.starts_with("cancelled[")
        || trimmed.starts_with("task_id.")
        || trimmed.starts_with("task_id[")
}

fn is_command_runner_scope_ref(text: &str) -> bool {
    is_value_scope_ref(text) || is_task_scope_ref(text)
}

fn binding_top_level_read_selector(binding: &model::WidgetBindingDef) -> Option<String> {
    let serde_yaml::Value::String(text) = binding.reads.as_ref()? else {
        return None;
    };
    if let Some(expr) = exact_template_expr(text) {
        return normalize_binding_selector(expr).ok();
    }
    normalize_binding_selector(text).ok()
}

fn write_expr_is_identity(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::String(text) => {
            let trimmed = text.trim();
            trimmed == "value" || exact_template_expr(trimmed).is_some_and(is_value_scope_ref)
        }
        _ => false,
    }
}

fn compile_text_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::TextOutput(model::TextOutputDef { id, text }) => {
            let node = outputs::compile_text_output(id, text.clone());
            if text.contains("{{") && text.contains("}}") {
                Ok(bind_node(
                    node,
                    StoreBinding {
                        reads: Some(ReadBinding::Template(text)),
                        ..StoreBinding::default()
                    },
                ))
            } else {
                Ok(node)
            }
        }
        _ => registry_dispatch_mismatch("text_output"),
    }
}

fn compile_url_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::UrlOutput(model::UrlOutputDef { id, url, name }) => {
            outputs::compile_url_output(id, url, name)
        }
        _ => registry_dispatch_mismatch("url_output"),
    }
}

fn compile_thinking_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ThinkingOutput(model::ThinkingOutputDef {
            id,
            label,
            text,
            mode,
            tail_len,
            tick_ms,
            base_rgb,
            peak_rgb,
        }) => outputs::compile_thinking_output(
            id, label, text, mode, tail_len, tick_ms, base_rgb, peak_rgb,
        ),
        _ => registry_dispatch_mismatch("thinking_output"),
    }
}

fn compile_progress_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ProgressOutput(model::ProgressOutputDef {
            id,
            label,
            min,
            max,
            unit,
            bar_width,
            style,
            transition,
        }) => outputs::compile_progress_output(
            id, label, min, max, unit, bar_width, style, transition,
        ),
        _ => registry_dispatch_mismatch("progress_output"),
    }
}

fn compile_chart_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ChartOutput(model::ChartOutputDef {
            id,
            label,
            mode,
            capacity,
            min,
            max,
            unit,
            gradient,
        }) => outputs::compile_chart_output(id, label, mode, capacity, min, max, unit, gradient),
        _ => registry_dispatch_mismatch("chart_output"),
    }
}

fn compile_table_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::TableOutput(model::TableOutputDef {
            id,
            label,
            style,
            headers,
            rows,
        }) => outputs::compile_table_output(id, label, style, headers, rows),
        _ => registry_dispatch_mismatch("table_output"),
    }
}

fn compile_diff_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::DiffOutput(model::DiffOutputDef {
            id,
            label,
            old,
            new,
            max_visible,
        }) => outputs::compile_diff_output(id, label, old, new, max_visible),
        _ => registry_dispatch_mismatch("diff_output"),
    }
}

fn compile_task_log_output_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::TaskLogOutput(model::TaskLogOutputDef {
            id,
            visible_lines,
            spinner_style,
            steps,
        }) => outputs::compile_task_log_output(id, visible_lines, spinner_style, steps),
        _ => registry_dispatch_mismatch("task_log_output"),
    }
}

fn compile_text_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::TextInput(model::TextInputDef {
            id,
            label,
            placeholder,
            default,
            mode,
            required,
            validators,
            completion_items,
            ..
        }) => inputs::compile_text_input(
            id,
            label,
            placeholder,
            default,
            mode,
            required,
            validators,
            completion_items,
        ),
        _ => registry_dispatch_mismatch("text_input"),
    }
}

fn compile_array_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ArrayInput(model::ArrayInputDef {
            id,
            label,
            items,
            required,
            validators,
            ..
        }) => inputs::compile_array_input(id, label, items, required, validators),
        _ => registry_dispatch_mismatch("array_input"),
    }
}

fn compile_button_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ButtonInput(model::ButtonInputDef {
            id,
            label,
            text,
            task_id,
        }) => inputs::compile_button_input(id, label, text, task_id),
        _ => registry_dispatch_mismatch("button_input"),
    }
}

fn compile_select_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Select(model::SelectDef {
            id,
            label,
            options,
            selected,
            default,
            required,
            validators,
            ..
        }) => inputs::compile_select_input(
            id,
            label,
            compile_string_options(options),
            selected,
            default,
            required,
            validators,
        ),
        _ => registry_dispatch_mismatch("select"),
    }
}

fn compile_choice_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ChoiceInput(model::ChoiceInputDef {
            id,
            label,
            options,
            bullets,
            default,
            required,
            validators,
            ..
        }) => inputs::compile_choice_input(
            id,
            label,
            compile_string_options(options),
            bullets,
            default,
            required,
            validators,
        ),
        _ => registry_dispatch_mismatch("choice_input"),
    }
}

fn compile_select_list_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::SelectList(model::SelectListDef {
            id,
            label,
            options,
            mode,
            max_visible,
            selected,
            show_label,
            ..
        }) => components::compile_select_list(
            id,
            label,
            compile_select_list_options(options),
            mode,
            max_visible,
            selected,
            show_label,
        ),
        _ => registry_dispatch_mismatch("select_list"),
    }
}

fn compile_masked_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::MaskedInput(model::MaskedInputDef {
            id,
            label,
            mask,
            default,
            required,
            validators,
            ..
        }) => inputs::compile_masked_input(id, label, mask, default, required, validators),
        _ => registry_dispatch_mismatch("masked_input"),
    }
}

fn compile_slider_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Slider(model::SliderDef {
            id,
            label,
            min,
            max,
            step,
            unit,
            track_len,
            default,
            required,
            validators,
            ..
        }) => inputs::compile_slider_input(
            id, label, min, max, step, unit, track_len, default, required, validators,
        ),
        _ => registry_dispatch_mismatch("slider"),
    }
}

fn compile_color_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ColorInput(model::ColorInputDef {
            id,
            label,
            rgb,
            required,
            validators,
            ..
        }) => inputs::compile_color_input(id, label, rgb, required, validators),
        _ => registry_dispatch_mismatch("color_input"),
    }
}

fn compile_confirm_input_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ConfirmInput(model::ConfirmInputDef {
            id,
            label,
            mode,
            yes_label,
            no_label,
            default,
            ..
        }) => inputs::compile_confirm_input(id, label, mode, yes_label, no_label, default),
        _ => registry_dispatch_mismatch("confirm_input"),
    }
}

fn compile_checkbox_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Checkbox(model::CheckboxDef {
            id,
            label,
            checked,
            required,
            validators,
            ..
        }) => inputs::compile_checkbox_input(id, label, checked, required, validators),
        _ => registry_dispatch_mismatch("checkbox"),
    }
}

fn compile_calendar_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Calendar(model::CalendarDef {
            id,
            label,
            mode,
            required,
            validators,
            ..
        }) => components::compile_calendar(id, label, mode, required, validators),
        _ => registry_dispatch_mismatch("calendar"),
    }
}

fn compile_textarea_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Textarea(model::TextareaDef {
            id,
            min_height,
            max_height,
            default,
            required,
            validators,
            ..
        }) => {
            components::compile_textarea(id, min_height, max_height, default, required, validators)
        }
        _ => registry_dispatch_mismatch("textarea"),
    }
}

fn compile_command_runner_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::CommandRunner(model::CommandRunnerDef {
            id,
            label,
            run_mode,
            on_error,
            advance_on_success,
            visible_lines,
            spinner_style,
            timeout_ms,
            commands,
            ..
        }) => components::compile_command_runner(
            id,
            label,
            run_mode,
            on_error,
            advance_on_success,
            visible_lines,
            spinner_style,
            timeout_ms,
            commands,
        ),
        _ => registry_dispatch_mismatch("command_runner"),
    }
}

fn compile_file_browser_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::FileBrowser(model::FileBrowserDef {
            id,
            label,
            browser_mode,
            selection_mode,
            entry_filter,
            display_mode,
            value_mode,
            cwd,
            recursive,
            hide_hidden,
            ext_filter,
            max_visible,
            required,
            validators,
            ..
        }) => components::compile_file_browser(
            id,
            label,
            browser_mode,
            selection_mode,
            entry_filter,
            display_mode,
            value_mode,
            cwd,
            recursive,
            hide_hidden,
            ext_filter,
            max_visible,
            required,
            validators,
        ),
        _ => registry_dispatch_mismatch("file_browser"),
    }
}

fn compile_tree_view_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::TreeView(model::TreeViewDef {
            id,
            label,
            nodes,
            max_visible,
            show_label,
            indent_guides,
            ..
        }) => {
            components::compile_tree_view(id, label, nodes, max_visible, show_label, indent_guides)
        }
        _ => registry_dispatch_mismatch("tree_view"),
    }
}

fn compile_object_editor_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::ObjectEditor(model::ObjectEditorDef {
            id,
            label,
            value,
            max_visible,
            ..
        }) => components::compile_object_editor(id, label, value, max_visible),
        _ => registry_dispatch_mismatch("object_editor"),
    }
}

fn compile_snippet_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Snippet(model::SnippetDef {
            id,
            label,
            template,
            inputs,
            ..
        }) => components::compile_snippet(id, label, template, inputs),
        _ => registry_dispatch_mismatch("snippet"),
    }
}

fn compile_table_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Table(model::TableDef {
            id,
            label,
            style,
            row_numbers,
            initial_rows,
            columns,
            ..
        }) => components::compile_table(id, label, style, row_numbers, initial_rows, columns),
        _ => registry_dispatch_mismatch("table"),
    }
}

fn compile_repeater_widget(def: WidgetDef) -> Result<Node, String> {
    match def {
        WidgetDef::Repeater(model::RepeaterDef {
            id,
            label,
            mode,
            layout,
            show_label,
            show_progress,
            header_template,
            item_label_path,
            items,
            count,
            widgets,
            ..
        }) => components::compile_repeater(
            id,
            label,
            mode,
            layout,
            show_label,
            show_progress,
            header_template,
            item_label_path,
            items,
            count,
            widgets,
        ),
        _ => registry_dispatch_mismatch("repeater"),
    }
}
