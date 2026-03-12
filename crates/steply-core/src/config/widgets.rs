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

macro_rules! widget_binding_value {
    (no, $def:ident) => {
        None
    };
    (yes, $def:ident) => {
        Some(&$def.binding)
    };
}

macro_rules! widget_children_value {
    (none, $def:ident) => {
        None
    };
    (inputs, $def:ident) => {
        Some($def.inputs.as_slice())
    };
    (widgets, $def:ident) => {
        Some($def.widgets.as_slice())
    };
}

macro_rules! define_widget_registry {
    (
        $(
            {
                variant: $variant:ident,
                def: $def_ty:ty,
                type_name: $type_name:literal,
                category: $category:ident,
                short: $short:literal,
                long: $long:literal,
                example: $example:literal,
                hints: $hints:expr,
                compile: $compile:ident,
                binding: $binding:ident,
                children: $children:ident
            }
        ),+ $(,)?
    ) => {
        const WIDGET_REGISTRY: &[WidgetRegistryEntry] = &[
            $(
                WidgetRegistryEntry {
                    doc: widget_doc(
                        $type_name,
                        WidgetCategory::$category,
                        $short,
                        $long,
                        $example,
                        $hints,
                    ),
                    build_doc: build_widget_doc::<$def_ty>,
                    compile: $compile,
                },
            )+
        ];

        impl WidgetDef {
            fn registry_type_name(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => $type_name,)+
                }
            }

            fn registry_id(&self) -> &str {
                match self {
                    $(Self::$variant(def) => def.id.as_str(),)+
                }
            }

            fn registry_binding(&self) -> Option<&model::WidgetBindingDef> {
                match self {
                    $(Self::$variant(_def) => widget_binding_value!($binding, _def),)+
                }
            }

            fn registry_children(&self) -> Option<&[WidgetDef]> {
                match self {
                    $(Self::$variant(_def) => widget_children_value!($children, _def),)+
                }
            }
        }
    };
}

define_widget_registry! {
    {
        variant: TextOutput,
        def: model::TextOutputDef,
        type_name: "text_output",
        category: Output,
        short: "Static text output.",
        long: "Renders a fixed text block in the flow.",
        example: r#"type: text_output
id: intro
text: Welcome to Steply"#,
        hints: &[],
        compile: compile_text_output_widget,
        binding: no,
        children: none
    },
    {
        variant: UrlOutput,
        def: model::UrlOutputDef,
        type_name: "url_output",
        category: Output,
        short: "Clickable URL output.",
        long: "Displays a URL with an optional human-friendly label.",
        example: r#"type: url_output
id: docs
url: https://example.com
name: Open docs"#,
        hints: &[],
        compile: compile_url_output_widget,
        binding: no,
        children: none
    },
    {
        variant: ThinkingOutput,
        def: model::ThinkingOutputDef,
        type_name: "thinking_output",
        category: Output,
        short: "Animated thinking output.",
        long: "Shows animated status text for background work or waiting states.",
        example: r#"type: thinking_output
id: thinking
label: Preparing
text: Resolving inputs"#,
        hints: &[],
        compile: compile_thinking_output_widget,
        binding: no,
        children: none
    },
    {
        variant: ProgressOutput,
        def: model::ProgressOutputDef,
        type_name: "progress_output",
        category: Output,
        short: "Progress bar output.",
        long: "Displays progress with optional range, style and transition settings.",
        example: r#"type: progress_output
id: build_progress
label: Build
min: 0
max: 100"#,
        hints: &[],
        compile: compile_progress_output_widget,
        binding: no,
        children: none
    },
    {
        variant: ChartOutput,
        def: model::ChartOutputDef,
        type_name: "chart_output",
        category: Output,
        short: "Terminal chart output.",
        long: "Renders numeric values as a small terminal chart.",
        example: r#"type: chart_output
id: cpu
label: CPU load"#,
        hints: &[],
        compile: compile_chart_output_widget,
        binding: no,
        children: none
    },
    {
        variant: TableOutput,
        def: model::TableOutputDef,
        type_name: "table_output",
        category: Output,
        short: "Read-only table output.",
        long: "Displays tabular data without inline editing.",
        example: r#"type: table_output
id: summary
label: Summary
headers: [Name, Value]"#,
        hints: &[],
        compile: compile_table_output_widget,
        binding: no,
        children: none
    },
    {
        variant: DiffOutput,
        def: model::DiffOutputDef,
        type_name: "diff_output",
        category: Output,
        short: "Text diff output.",
        long: "Shows the difference between two text values.",
        example: r#"type: diff_output
id: diff
label: Planned changes
old: before
new: after"#,
        hints: &[],
        compile: compile_diff_output_widget,
        binding: no,
        children: none
    },
    {
        variant: TaskLogOutput,
        def: model::TaskLogOutputDef,
        type_name: "task_log_output",
        category: Output,
        short: "Task log output.",
        long: "Displays task execution progress as a step-by-step log.",
        example: r#"type: task_log_output
id: task_log
steps:
  - label: Clone repo
    task_id: clone"#,
        hints: &[],
        compile: compile_task_log_output_widget,
        binding: no,
        children: none
    },
    {
        variant: TextInput,
        def: model::TextInputDef,
        type_name: "text_input",
        category: Input,
        short: "Single-line text input.",
        long: "Collects one line of text with optional validation and completion.",
        example: r#"type: text_input
id: project_name
label: Project name"#,
        hints: &[],
        compile: compile_text_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: ArrayInput,
        def: model::ArrayInputDef,
        type_name: "array_input",
        category: Input,
        short: "Array input.",
        long: "Collects multiple string values as a list.",
        example: r#"type: array_input
id: tags
label: Tags"#,
        hints: &[],
        compile: compile_array_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: ButtonInput,
        def: model::ButtonInputDef,
        type_name: "button_input",
        category: Input,
        short: "Button input.",
        long: "Focusable button that may trigger a task when activated.",
        example: r#"type: button_input
id: refresh
label: Refresh"#,
        hints: &[],
        compile: compile_button_input_widget,
        binding: no,
        children: none
    },
    {
        variant: Select,
        def: model::SelectDef,
        type_name: "select",
        category: Input,
        short: "Single-select input.",
        long: "Lets the user choose one option from a list.",
        example: r#"type: select
id: region
label: Region
options: [eu, us]"#,
        hints: &[],
        compile: compile_select_widget,
        binding: yes,
        children: none
    },
    {
        variant: ChoiceInput,
        def: model::ChoiceInputDef,
        type_name: "choice_input",
        category: Input,
        short: "Choice input.",
        long: "Choice selector rendered as navigable options.",
        example: r#"type: choice_input
id: package_manager
label: Package manager
options: [cargo, npm]"#,
        hints: static_hints::CHOICE_INPUT_HINTS,
        compile: compile_choice_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: SelectList,
        def: model::SelectListDef,
        type_name: "select_list",
        category: Component,
        short: "Selectable list component.",
        long: "Interactive list component supporting single or multi selection.",
        example: r#"type: select_list
id: features
label: Features
options: [auth, api]"#,
        hints: static_hints::SELECT_LIST_DOC_HINTS,
        compile: compile_select_list_widget,
        binding: yes,
        children: none
    },
    {
        variant: MaskedInput,
        def: model::MaskedInputDef,
        type_name: "masked_input",
        category: Input,
        short: "Masked input.",
        long: "Text input constrained by a mask pattern.",
        example: r#"type: masked_input
id: phone
label: Phone
mask: \"(999) 999-9999\""#,
        hints: &[],
        compile: compile_masked_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: Slider,
        def: model::SliderDef,
        type_name: "slider",
        category: Input,
        short: "Slider input.",
        long: "Numeric slider with configurable range and step.",
        example: r#"type: slider
id: retries
label: Retries
min: 0
max: 10"#,
        hints: &[],
        compile: compile_slider_widget,
        binding: yes,
        children: none
    },
    {
        variant: ColorInput,
        def: model::ColorInputDef,
        type_name: "color_input",
        category: Input,
        short: "Color input.",
        long: "Picks a color represented as RGB values.",
        example: r#"type: color_input
id: accent
label: Accent color"#,
        hints: &[],
        compile: compile_color_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: ConfirmInput,
        def: model::ConfirmInputDef,
        type_name: "confirm_input",
        category: Input,
        short: "Confirmation input.",
        long: "Confirms a yes/no choice in relaxed or strict mode.",
        example: r#"type: confirm_input
id: proceed
label: Continue?"#,
        hints: static_hints::CONFIRM_STRICT_HINTS,
        compile: compile_confirm_input_widget,
        binding: yes,
        children: none
    },
    {
        variant: Checkbox,
        def: model::CheckboxDef,
        type_name: "checkbox",
        category: Input,
        short: "Checkbox input.",
        long: "Single boolean checkbox control.",
        example: r#"type: checkbox
id: accept
label: Accept terms"#,
        hints: &[],
        compile: compile_checkbox_widget,
        binding: yes,
        children: none
    },
    {
        variant: Calendar,
        def: model::CalendarDef,
        type_name: "calendar",
        category: Component,
        short: "Calendar component.",
        long: "Interactive date, time or date-time picker.",
        example: r#"type: calendar
id: deploy_at
label: Deploy at"#,
        hints: static_hints::CALENDAR_COMMON_HINTS,
        compile: compile_calendar_widget,
        binding: yes,
        children: none
    },
    {
        variant: Textarea,
        def: model::TextareaDef,
        type_name: "textarea",
        category: Component,
        short: "Textarea component.",
        long: "Multi-line text editor widget.",
        example: r#"type: textarea
id: notes
min_height: 4"#,
        hints: static_hints::TEXTAREA_HINTS,
        compile: compile_textarea_widget,
        binding: yes,
        children: none
    },
    {
        variant: CommandRunner,
        def: model::CommandRunnerDef,
        type_name: "command_runner",
        category: Component,
        short: "Command runner.",
        long: "Runs one or more shell commands inside the flow.",
        example: r#"type: command_runner
id: install
label: Install dependencies
commands:
  - label: Cargo fetch
    program: cargo
    args: [fetch]"#,
        hints: static_hints::COMMAND_RUNNER_HINTS,
        compile: compile_command_runner_widget,
        binding: yes,
        children: none
    },
    {
        variant: FileBrowser,
        def: model::FileBrowserDef,
        type_name: "file_browser",
        category: Component,
        short: "File browser component.",
        long: "Interactive file or directory browser with optional completion.",
        example: r#"type: file_browser
id: project_dir
label: Select project directory"#,
        hints: static_hints::FILE_BROWSER_DOC_HINTS,
        compile: compile_file_browser_widget,
        binding: yes,
        children: none
    },
    {
        variant: TreeView,
        def: model::TreeViewDef,
        type_name: "tree_view",
        category: Component,
        short: "Tree view component.",
        long: "Interactive hierarchical tree selector.",
        example: r#"type: tree_view
id: modules
label: Modules
nodes:
  - item: core
    depth: 0
    has_children: true"#,
        hints: static_hints::TREE_VIEW_DOC_HINTS,
        compile: compile_tree_view_widget,
        binding: yes,
        children: none
    },
    {
        variant: ObjectEditor,
        def: model::ObjectEditorDef,
        type_name: "object_editor",
        category: Component,
        short: "Object editor.",
        long: "Structured editor for object-like values.",
        example: r#"type: object_editor
id: payload
label: Payload"#,
        hints: static_hints::OBJECT_EDITOR_DOC_HINTS,
        compile: compile_object_editor_widget,
        binding: yes,
        children: none
    },
    {
        variant: Snippet,
        def: model::SnippetDef,
        type_name: "snippet",
        category: Component,
        short: "Snippet component.",
        long: "Builds a snippet from nested interactive inputs.",
        example: r#"type: snippet
id: export_cmd
label: Export command
template: \"export NAME={{name}}\""#,
        hints: static_hints::SNIPPET_HINTS,
        compile: compile_snippet_widget,
        binding: yes,
        children: inputs
    },
    {
        variant: Table,
        def: model::TableDef,
        type_name: "table",
        category: Component,
        short: "Editable table component.",
        long: "Edits repeated rows using embedded widgets per column.",
        example: r#"type: table
id: envs
label: Environments
columns:
  - header: Name
    widget:
      type: text_input"#,
        hints: static_hints::TABLE_DOC_HINTS,
        compile: compile_table_widget,
        binding: yes,
        children: none
    },
    {
        variant: Repeater,
        def: model::RepeaterDef,
        type_name: "repeater",
        category: Component,
        short: "Repeater component.",
        long: "Edits repeated items using embedded widget fields.",
        example: r#"type: repeater
id: services
label: Services
fields:
  - key: name
    label: Name
    widget:
      type: text_input"#,
        hints: static_hints::REPEATER_HINTS,
        compile: compile_repeater_widget,
        binding: yes,
        children: widgets
    }
}

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
        if let Some(children) = widget.registry_children() {
            walk_widgets(children, visitor)?;
        }
    }
    Ok(())
}

pub(super) fn widget_id(widget: &WidgetDef) -> &str {
    widget.registry_id()
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
    let Some(binding) = widget.registry_binding() else {
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
    let Some(binding) = widget.registry_binding() else {
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
    let Some(binding) = widget.registry_binding() else {
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
    let widget_type = def.registry_type_name();
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

fn registry_dispatch_mismatch<T>(widget_type: &str) -> Result<T, String> {
    Err(format!(
        "internal widget registry dispatch mismatch for '{widget_type}'"
    ))
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
    let mut binding = compile_widget_binding(def, def.registry_binding().cloned())?;
    binding.options = compile_option_binding(def)?;
    Ok(binding)
}

pub(super) fn compile_task_writes(
    writes: Option<model::WriteBindingDef>,
) -> Result<Vec<WriteBinding>, String> {
    compile_write_bindings(writes, "result", is_task_scope_ref)
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
    trimmed == "result" || trimmed.starts_with("result.") || trimmed.starts_with("result[")
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
            default,
            max_visible,
            ..
        }) => components::compile_object_editor(id, label, default, max_visible),
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
