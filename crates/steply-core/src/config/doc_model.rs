use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::widgets::static_hints;
use crate::widgets::traits::StaticHintSpec;

use super::model::*;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDocs {
    pub version: u32,
    pub widgets: Vec<WidgetDoc>,
    pub embedded_widgets: Vec<WidgetDoc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetDoc {
    pub widget_type: &'static str,
    pub category: WidgetCategory,
    pub short_description: &'static str,
    pub long_description: &'static str,
    pub example_yaml: &'static str,
    pub static_hints: Vec<StaticHintSpec>,
    pub fields: Vec<FieldDoc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetCategory {
    Output,
    Input,
    Component,
    Embedded,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldDoc {
    pub name: String,
    pub type_name: String,
    pub required: bool,
    pub short_description: String,
    pub long_description: Option<String>,
    pub default: Option<String>,
    pub allowed_values: Vec<String>,
}

pub trait DocumentedModel: JsonSchema {
    const TYPE_NAME: &'static str;
    const CATEGORY: WidgetCategory;
    const SHORT_DESCRIPTION: &'static str;
    const LONG_DESCRIPTION: &'static str;
    const EXAMPLE_YAML: &'static str;

    fn static_hints() -> &'static [StaticHintSpec] {
        &[]
    }
}

pub fn schema_docs() -> Result<ConfigDocs, String> {
    Ok(ConfigDocs {
        version: 1,
        widgets: vec![
            build_widget_doc::<TextOutputDef>()?,
            build_widget_doc::<UrlOutputDef>()?,
            build_widget_doc::<ThinkingOutputDef>()?,
            build_widget_doc::<ProgressOutputDef>()?,
            build_widget_doc::<ChartOutputDef>()?,
            build_widget_doc::<TableOutputDef>()?,
            build_widget_doc::<DiffOutputDef>()?,
            build_widget_doc::<TaskLogOutputDef>()?,
            build_widget_doc::<TextInputDef>()?,
            build_widget_doc::<ArrayInputDef>()?,
            build_widget_doc::<ButtonInputDef>()?,
            build_widget_doc::<SelectDef>()?,
            build_widget_doc::<ChoiceInputDef>()?,
            build_widget_doc::<SelectListDef>()?,
            build_widget_doc::<MaskedInputDef>()?,
            build_widget_doc::<SliderDef>()?,
            build_widget_doc::<ColorInputDef>()?,
            build_widget_doc::<ConfirmInputDef>()?,
            build_widget_doc::<CheckboxDef>()?,
            build_widget_doc::<CalendarDef>()?,
            build_widget_doc::<TextareaDef>()?,
            build_widget_doc::<CommandRunnerDef>()?,
            build_widget_doc::<FileBrowserDef>()?,
            build_widget_doc::<TreeViewDef>()?,
            build_widget_doc::<ObjectEditorDef>()?,
            build_widget_doc::<SnippetDef>()?,
            build_widget_doc::<TableDef>()?,
            build_widget_doc::<RepeaterDef>()?,
        ],
        embedded_widgets: vec![
            build_widget_doc::<EmbeddedTextInputDef>()?,
            build_widget_doc::<EmbeddedMaskedInputDef>()?,
            build_widget_doc::<EmbeddedSelectDef>()?,
            build_widget_doc::<EmbeddedSliderDef>()?,
            build_widget_doc::<EmbeddedCheckboxDef>()?,
            build_widget_doc::<EmbeddedArrayInputDef>()?,
        ],
    })
}

pub fn schema_docs_json() -> Result<String, String> {
    let docs = schema_docs()?;
    serde_json::to_string_pretty(&docs)
        .map_err(|err| format!("failed to serialize config docs: {err}"))
}

pub fn yaml_value_schema(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    <serde_json::Value as JsonSchema>::json_schema(generator)
}

fn build_widget_doc<T: DocumentedModel>() -> Result<WidgetDoc, String> {
    Ok(WidgetDoc {
        widget_type: T::TYPE_NAME,
        category: T::CATEGORY,
        short_description: T::SHORT_DESCRIPTION,
        long_description: T::LONG_DESCRIPTION,
        example_yaml: T::EXAMPLE_YAML,
        static_hints: T::static_hints().to_vec(),
        fields: extract_field_docs::<T>()?,
    })
}

fn extract_field_docs<T: JsonSchema>() -> Result<Vec<FieldDoc>, String> {
    let schema_json = serde_json::to_value(schema_for!(T))
        .map_err(|err| format!("failed to serialize schema: {err}"))?;
    let root = schema_json
        .as_object()
        .ok_or_else(|| "invalid schema root".to_string())?;
    let defs = root
        .get("definitions")
        .or_else(|| root.get("$defs"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let schema_object = if let Some(schema) = root.get("schema").and_then(Value::as_object) {
        schema
    } else {
        root
    };

    let properties = schema_object
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| "schema does not describe an object".to_string())?;
    let required = schema_object
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();

    let mut fields = properties
        .iter()
        .map(|(name, property)| FieldDoc {
            name: name.clone(),
            type_name: schema_type_name(property, &defs),
            required: required.contains(name.as_str()),
            short_description: split_description(
                property
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .0,
            long_description: split_description(
                property
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .1,
            default: property.get("default").map(value_to_string),
            allowed_values: property
                .get("enum")
                .and_then(Value::as_array)
                .map(|values| values.iter().map(value_to_string).collect())
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fields)
}

fn schema_type_name(schema: &Value, defs: &Map<String, Value>) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(name) = reference
            .strip_prefix("#/definitions/")
            .or_else(|| reference.strip_prefix("#/$defs/"))
    {
        if let Some(target) = defs.get(name) {
            return schema_type_name(target, defs);
        }
        return name.to_string();
    }

    if let Some(type_name) = schema.get("type").and_then(Value::as_str) {
        return match type_name {
            "array" => {
                let inner = schema
                    .get("items")
                    .map(|items| schema_type_name(items, defs))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("list<{inner}>")
            }
            "integer" => "number".to_string(),
            other => other.to_string(),
        };
    }

    if let Some(type_names) = schema.get("type").and_then(Value::as_array) {
        let names = type_names
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| *name != "null")
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return names.join(" | ");
        }
    }

    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(items) = schema.get(key).and_then(Value::as_array) {
            let names = items
                .iter()
                .map(|item| schema_type_name(item, defs))
                .filter(|name| name != "null")
                .collect::<Vec<_>>();
            if !names.is_empty() {
                return names.join(" | ");
            }
        }
    }

    if schema.get("properties").is_some() {
        return "object".to_string();
    }

    "unknown".to_string()
}

fn split_description(description: &str) -> (String, Option<String>) {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let mut parts = trimmed.splitn(2, "\n\n");
    let short = parts.next().unwrap_or_default().trim().to_string();
    let long = parts
        .next()
        .map(|rest| rest.trim().to_string())
        .filter(|s| !s.is_empty());
    (short, long)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

macro_rules! impl_doc_model {
    ($ty:ty, $type_name:literal, $category:expr, $short:literal, $long:literal, $example:expr, $hints:expr) => {
        impl DocumentedModel for $ty {
            const TYPE_NAME: &'static str = $type_name;
            const CATEGORY: WidgetCategory = $category;
            const SHORT_DESCRIPTION: &'static str = $short;
            const LONG_DESCRIPTION: &'static str = $long;
            const EXAMPLE_YAML: &'static str = $example;

            fn static_hints() -> &'static [StaticHintSpec] {
                $hints
            }
        }
    };
}

impl_doc_model!(
    TextOutputDef,
    "text_output",
    WidgetCategory::Output,
    "Static text output.",
    "Renders a fixed text block in the flow.",
    r#"type: text_output
id: intro
text: Welcome to Steply"#,
    &[]
);
impl_doc_model!(
    UrlOutputDef,
    "url_output",
    WidgetCategory::Output,
    "Clickable URL output.",
    "Displays a URL with an optional human-friendly label.",
    r#"type: url_output
id: docs
url: https://example.com
name: Open docs"#,
    &[]
);
impl_doc_model!(
    ThinkingOutputDef,
    "thinking_output",
    WidgetCategory::Output,
    "Animated thinking output.",
    "Shows animated status text for background work or waiting states.",
    r#"type: thinking_output
id: thinking
label: Preparing
text: Resolving inputs"#,
    &[]
);
impl_doc_model!(
    ProgressOutputDef,
    "progress_output",
    WidgetCategory::Output,
    "Progress bar output.",
    "Displays progress with optional range, style and transition settings.",
    r#"type: progress_output
id: build_progress
label: Build
min: 0
max: 100"#,
    &[]
);
impl_doc_model!(
    ChartOutputDef,
    "chart_output",
    WidgetCategory::Output,
    "Terminal chart output.",
    "Renders numeric values as a small terminal chart.",
    r#"type: chart_output
id: cpu
label: CPU load"#,
    &[]
);
impl_doc_model!(
    TableOutputDef,
    "table_output",
    WidgetCategory::Output,
    "Read-only table output.",
    "Displays tabular data without inline editing.",
    r#"type: table_output
id: summary
label: Summary
headers: [Name, Value]"#,
    &[]
);
impl_doc_model!(
    DiffOutputDef,
    "diff_output",
    WidgetCategory::Output,
    "Text diff output.",
    "Shows the difference between two text values.",
    r#"type: diff_output
id: diff
label: Planned changes
old: before
new: after"#,
    &[]
);
impl_doc_model!(
    TaskLogOutputDef,
    "task_log_output",
    WidgetCategory::Output,
    "Task log output.",
    "Displays task execution progress as a step-by-step log.",
    r#"type: task_log_output
id: task_log
steps:
  - label: Clone repo
    task_id: clone"#,
    &[]
);
impl_doc_model!(
    TextInputDef,
    "text_input",
    WidgetCategory::Input,
    "Single-line text input.",
    "Collects one line of text with optional validation and completion.",
    r#"type: text_input
id: project_name
label: Project name"#,
    &[]
);
impl_doc_model!(
    ArrayInputDef,
    "array_input",
    WidgetCategory::Input,
    "Array input.",
    "Collects multiple string values as a list.",
    r#"type: array_input
id: tags
label: Tags"#,
    &[]
);
impl_doc_model!(
    ButtonInputDef,
    "button_input",
    WidgetCategory::Input,
    "Button input.",
    "Focusable button that may trigger a task when activated.",
    r#"type: button_input
id: refresh
label: Refresh"#,
    &[]
);
impl_doc_model!(
    SelectDef,
    "select",
    WidgetCategory::Input,
    "Single-select input.",
    "Lets the user choose one option from a list.",
    r#"type: select
id: region
label: Region
options: [eu, us]"#,
    &[]
);
impl_doc_model!(
    ChoiceInputDef,
    "choice_input",
    WidgetCategory::Input,
    "Choice input.",
    "Choice selector rendered as navigable options.",
    r#"type: choice_input
id: package_manager
label: Package manager
options: [cargo, npm]"#,
    static_hints::CHOICE_INPUT_HINTS
);
impl_doc_model!(
    SelectListDef,
    "select_list",
    WidgetCategory::Component,
    "Selectable list component.",
    "Interactive list component supporting single or multi selection.",
    r#"type: select_list
id: features
label: Features
options: [auth, api]"#,
    static_hints::SELECT_LIST_DOC_HINTS
);
impl_doc_model!(
    MaskedInputDef,
    "masked_input",
    WidgetCategory::Input,
    "Masked input.",
    "Text input constrained by a mask pattern.",
    r#"type: masked_input
id: phone
label: Phone
mask: \"(999) 999-9999\""#,
    &[]
);
impl_doc_model!(
    SliderDef,
    "slider",
    WidgetCategory::Input,
    "Slider input.",
    "Numeric slider with configurable range and step.",
    r#"type: slider
id: retries
label: Retries
min: 0
max: 10"#,
    &[]
);
impl_doc_model!(
    ColorInputDef,
    "color_input",
    WidgetCategory::Input,
    "Color input.",
    "Picks a color represented as RGB values.",
    r#"type: color_input
id: accent
label: Accent color"#,
    &[]
);
impl_doc_model!(
    ConfirmInputDef,
    "confirm_input",
    WidgetCategory::Input,
    "Confirmation input.",
    "Confirms a yes/no choice in relaxed or strict mode.",
    r#"type: confirm_input
id: proceed
label: Continue?"#,
    static_hints::CONFIRM_STRICT_HINTS
);
impl_doc_model!(
    CheckboxDef,
    "checkbox",
    WidgetCategory::Input,
    "Checkbox input.",
    "Single boolean checkbox control.",
    r#"type: checkbox
id: accept
label: Accept terms"#,
    &[]
);
impl_doc_model!(
    CalendarDef,
    "calendar",
    WidgetCategory::Component,
    "Calendar component.",
    "Interactive date, time or date-time picker.",
    r#"type: calendar
id: deploy_at
label: Deploy at"#,
    static_hints::CALENDAR_COMMON_HINTS
);
impl_doc_model!(
    TextareaDef,
    "textarea",
    WidgetCategory::Component,
    "Textarea component.",
    "Multi-line text editor widget.",
    r#"type: textarea
id: notes
min_height: 4"#,
    static_hints::TEXTAREA_HINTS
);
impl_doc_model!(
    CommandRunnerDef,
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
    static_hints::COMMAND_RUNNER_HINTS
);
impl_doc_model!(
    FileBrowserDef,
    "file_browser",
    WidgetCategory::Component,
    "File browser component.",
    "Interactive file or directory browser with optional completion.",
    r#"type: file_browser
id: project_dir
label: Select project directory"#,
    static_hints::FILE_BROWSER_DOC_HINTS
);
impl_doc_model!(
    TreeViewDef,
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
    static_hints::TREE_VIEW_DOC_HINTS
);
impl_doc_model!(
    ObjectEditorDef,
    "object_editor",
    WidgetCategory::Component,
    "Object editor.",
    "Structured editor for object-like values.",
    r#"type: object_editor
id: payload
label: Payload"#,
    static_hints::OBJECT_EDITOR_DOC_HINTS
);
impl_doc_model!(
    SnippetDef,
    "snippet",
    WidgetCategory::Component,
    "Snippet component.",
    "Builds a snippet from nested interactive inputs.",
    r#"type: snippet
id: export_cmd
label: Export command
template: \"export NAME={{name}}\""#,
    static_hints::SNIPPET_HINTS
);
impl_doc_model!(
    TableDef,
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
    static_hints::TABLE_DOC_HINTS
);
impl_doc_model!(
    RepeaterDef,
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
    static_hints::REPEATER_HINTS
);

impl_doc_model!(
    EmbeddedTextInputDef,
    "text_input",
    WidgetCategory::Embedded,
    "Embedded text input.",
    "Single-line text input usable inside tables and repeaters.",
    r#"type: text_input
placeholder: service-name"#,
    &[]
);
impl_doc_model!(
    EmbeddedMaskedInputDef,
    "masked_input",
    WidgetCategory::Embedded,
    "Embedded masked input.",
    "Masked text input usable inside tables and repeaters.",
    r#"type: masked_input
mask: \"999-AAA\""#,
    &[]
);
impl_doc_model!(
    EmbeddedSelectDef,
    "select",
    WidgetCategory::Embedded,
    "Embedded select input.",
    "Embedded single-choice select field.",
    r#"type: select
options: [small, medium, large]"#,
    &[]
);
impl_doc_model!(
    EmbeddedSliderDef,
    "slider",
    WidgetCategory::Embedded,
    "Embedded slider input.",
    "Embedded numeric slider field.",
    r#"type: slider
min: 0
max: 100"#,
    &[]
);
impl_doc_model!(
    EmbeddedCheckboxDef,
    "checkbox",
    WidgetCategory::Embedded,
    "Embedded checkbox input.",
    "Embedded boolean checkbox field.",
    r#"type: checkbox
checked: true"#,
    &[]
);
impl_doc_model!(
    EmbeddedArrayInputDef,
    "array_input",
    WidgetCategory::Embedded,
    "Embedded array input.",
    "Embedded list input field.",
    r#"type: array_input
items: [a, b]"#,
    &[]
);
