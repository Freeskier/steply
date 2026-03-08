use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct ConfigDoc {
    #[serde(default)]
    pub(super) version: Option<u32>,
    #[serde(default)]
    pub(super) steps: Vec<StepDef>,
    #[serde(default)]
    pub(super) flow: Vec<FlowItemDef>,
    #[serde(default)]
    pub(super) tasks: Vec<TaskDef>,
    #[serde(default)]
    pub(super) subscriptions: Vec<SubscriptionDef>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct StepDef {
    pub(super) id: String,
    #[serde(alias = "prompt")]
    pub(super) title: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) when: Option<WhenDef>,
    #[serde(default)]
    pub(super) navigation: Option<NavigationDef>,
    #[serde(default)]
    pub(super) widgets: Vec<WidgetDef>,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum NavigationDef {
    Allowed,
    Locked,
    Reset,
    Destructive { warning: String },
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct FlowItemDef {
    pub(super) step: String,
    #[serde(default)]
    pub(super) when: Option<WhenDef>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct TaskDef {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) program: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
    #[serde(default)]
    pub(super) parse: Option<String>,
    #[serde(default)]
    pub(super) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(super) enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct SubscriptionDef {
    pub(super) task: String,
    pub(super) trigger: TriggerDef,
    #[serde(default)]
    pub(super) target: Option<String>,
    #[serde(default)]
    pub(super) enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct TriggerDef {
    #[serde(default)]
    pub(super) on_input: Option<OnInputTriggerDef>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct OnInputTriggerDef {
    #[serde(rename = "ref")]
    pub(super) field_ref: String,
    #[serde(default)]
    pub(super) debounce_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub(super) struct WhenDef {
    /// Store selector used by the condition.
    #[serde(default, rename = "ref")]
    pub(super) field_ref: Option<String>,
    /// Value that must be equal to the referenced value.
    #[serde(default)]
    #[schemars(schema_with = "super::doc_model::yaml_value_schema")]
    pub(super) equal: Option<serde_yaml::Value>,
    /// Value that must differ from the referenced value.
    #[serde(default)]
    #[schemars(schema_with = "super::doc_model::yaml_value_schema")]
    pub(super) not_equal: Option<serde_yaml::Value>,
    /// Whether the referenced value must be non-empty.
    #[serde(default)]
    pub(super) not_empty: Option<bool>,
    /// All nested conditions that must match.
    #[serde(default)]
    pub(super) all: Vec<WhenDef>,
    /// Any nested condition that may match.
    #[serde(default)]
    pub(super) any: Vec<WhenDef>,
    /// Nested condition that must not match.
    #[serde(default)]
    pub(super) not: Option<Box<WhenDef>>,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum WidgetDef {
    TextOutput(TextOutputDef),
    UrlOutput(UrlOutputDef),
    ThinkingOutput(ThinkingOutputDef),
    ProgressOutput(ProgressOutputDef),
    ChartOutput(ChartOutputDef),
    TableOutput(TableOutputDef),
    DiffOutput(DiffOutputDef),
    TaskLogOutput(TaskLogOutputDef),
    TextInput(TextInputDef),
    ArrayInput(ArrayInputDef),
    ButtonInput(ButtonInputDef),
    Select(SelectDef),
    ChoiceInput(ChoiceInputDef),
    SelectList(SelectListDef),
    MaskedInput(MaskedInputDef),
    Slider(SliderDef),
    ColorInput(ColorInputDef),
    ConfirmInput(ConfirmInputDef),
    Checkbox(CheckboxDef),
    Calendar(CalendarDef),
    Textarea(TextareaDef),
    CommandRunner(CommandRunnerDef),
    FileBrowser(FileBrowserDef),
    TreeView(TreeViewDef),
    ObjectEditor(ObjectEditorDef),
    Snippet(SnippetDef),
    Table(TableDef),
    Repeater(RepeaterDef),
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TextOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Rendered text content.
    pub(super) text: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct UrlOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Target URL.
    pub(super) url: String,
    /// Optional display label.
    #[serde(default)]
    pub(super) name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ThinkingOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Base text content.
    pub(super) text: String,
    /// Animation mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// Length of the animation tail.
    #[serde(default)]
    pub(super) tail_len: Option<usize>,
    /// Animation update interval in milliseconds.
    #[serde(default)]
    pub(super) tick_ms: Option<u64>,
    /// Base RGB color for the animation gradient.
    #[serde(default)]
    pub(super) base_rgb: Option<[u8; 3]>,
    /// Peak RGB color for the animation gradient.
    #[serde(default)]
    pub(super) peak_rgb: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ProgressOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Minimum progress value.
    #[serde(default)]
    pub(super) min: Option<f64>,
    /// Maximum progress value.
    #[serde(default)]
    pub(super) max: Option<f64>,
    /// Optional value suffix.
    #[serde(default)]
    pub(super) unit: Option<String>,
    /// Explicit bar width.
    #[serde(default)]
    pub(super) bar_width: Option<usize>,
    /// Progress rendering style.
    #[serde(default)]
    pub(super) style: Option<String>,
    /// Transition configuration for value changes.
    #[serde(default)]
    pub(super) transition: Option<ProgressTransitionDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ChartOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Chart render mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// Maximum number of points retained.
    #[serde(default)]
    pub(super) capacity: Option<usize>,
    /// Minimum chart range.
    #[serde(default)]
    pub(super) min: Option<f64>,
    /// Maximum chart range.
    #[serde(default)]
    pub(super) max: Option<f64>,
    /// Optional value suffix.
    #[serde(default)]
    pub(super) unit: Option<String>,
    /// Enables gradient coloring.
    #[serde(default)]
    pub(super) gradient: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TableOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Table rendering style.
    #[serde(default)]
    pub(super) style: Option<String>,
    /// Column headers.
    #[serde(default)]
    pub(super) headers: Vec<String>,
    /// Table rows.
    #[serde(default)]
    pub(super) rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct DiffOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Previous text.
    pub(super) old: String,
    /// Updated text.
    pub(super) new: String,
    /// Maximum number of visible diff lines.
    #[serde(default)]
    pub(super) max_visible: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TaskLogOutputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Maximum number of rendered log lines.
    #[serde(default)]
    pub(super) visible_lines: Option<usize>,
    /// Spinner style used while tasks are running.
    #[serde(default)]
    pub(super) spinner_style: Option<String>,
    /// Task log steps with label and task id.
    pub(super) steps: Vec<TaskLogStepDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TextInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Placeholder text shown when the field is empty.
    #[serde(default)]
    pub(super) placeholder: Option<String>,
    /// Initial field value.
    #[serde(default)]
    pub(super) default: Option<String>,
    /// Text display mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Static completion candidates.
    #[serde(default)]
    pub(super) completion_items: Vec<String>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
    /// Store selectors updated on each change.
    #[serde(default)]
    pub(super) change_targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ArrayInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Initial string items.
    #[serde(default)]
    pub(super) items: Vec<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ButtonInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Optional button text override.
    #[serde(default)]
    pub(super) text: Option<String>,
    /// Optional task executed when activated.
    #[serde(default)]
    pub(super) task_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct SelectDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Available option values.
    pub(super) options: Vec<String>,
    /// Initially selected option index.
    #[serde(default)]
    pub(super) selected: Option<usize>,
    /// Default option value.
    #[serde(default)]
    pub(super) default: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ChoiceInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Available option values.
    pub(super) options: Vec<String>,
    /// Whether to show bullet markers.
    #[serde(default)]
    pub(super) bullets: Option<bool>,
    /// Default option value.
    #[serde(default)]
    pub(super) default: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct SelectListDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// List items, plain or detailed.
    #[serde(default)]
    pub(super) options: Vec<SelectListOptionDef>,
    /// Selection mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// Maximum number of visible rows.
    #[serde(default)]
    pub(super) max_visible: Option<usize>,
    /// Initially selected indices.
    #[serde(default)]
    pub(super) selected: Vec<usize>,
    /// Whether to render the label.
    #[serde(default)]
    pub(super) show_label: Option<bool>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct MaskedInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Mask pattern.
    pub(super) mask: String,
    /// Initial field value.
    #[serde(default)]
    pub(super) default: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct SliderDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Minimum value.
    pub(super) min: i64,
    /// Maximum value.
    pub(super) max: i64,
    /// Increment step.
    #[serde(default)]
    pub(super) step: Option<i64>,
    /// Optional display unit.
    #[serde(default)]
    pub(super) unit: Option<String>,
    /// Rendered track length.
    #[serde(default)]
    pub(super) track_len: Option<usize>,
    /// Initial numeric value.
    #[serde(default)]
    pub(super) default: Option<f64>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selectors updated on each change.
    #[serde(default)]
    pub(super) change_targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ColorInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Initial RGB value.
    #[serde(default)]
    pub(super) rgb: Option<[u8; 3]>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ConfirmInputDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Confirmation mode.
    #[serde(default)]
    pub(super) mode: Option<ConfirmModeDef>,
    /// Custom yes label.
    #[serde(default)]
    pub(super) yes_label: Option<String>,
    /// Custom no label.
    #[serde(default)]
    pub(super) no_label: Option<String>,
    /// Initial boolean value.
    #[serde(default)]
    pub(super) default: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct CheckboxDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Initial checked state.
    #[serde(default)]
    pub(super) checked: Option<bool>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct CalendarDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Calendar mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TextareaDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Minimum visible height.
    #[serde(default)]
    pub(super) min_height: Option<usize>,
    /// Maximum visible height.
    #[serde(default)]
    pub(super) max_height: Option<usize>,
    /// Initial text value.
    #[serde(default)]
    pub(super) default: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct CommandRunnerDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Execution mode.
    #[serde(default)]
    pub(super) run_mode: Option<String>,
    /// Behavior when a command fails.
    #[serde(default)]
    pub(super) on_error: Option<String>,
    /// Whether to advance the step on success.
    #[serde(default)]
    pub(super) advance_on_success: Option<bool>,
    /// Maximum number of rendered log lines.
    #[serde(default)]
    pub(super) visible_lines: Option<usize>,
    /// Spinner style used during execution.
    #[serde(default)]
    pub(super) spinner_style: Option<String>,
    /// Command timeout in milliseconds.
    #[serde(default)]
    pub(super) timeout_ms: Option<u64>,
    /// Commands executed by the runner.
    pub(super) commands: Vec<CommandRunnerCommandDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct FileBrowserDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Browser mode.
    #[serde(default)]
    pub(super) browser_mode: Option<String>,
    /// Path display mode.
    #[serde(default)]
    pub(super) display_mode: Option<String>,
    /// Starting directory.
    #[serde(default)]
    pub(super) cwd: Option<String>,
    /// Whether to recurse into subdirectories.
    #[serde(default)]
    pub(super) recursive: Option<bool>,
    /// Whether to hide hidden entries.
    #[serde(default)]
    pub(super) hide_hidden: Option<bool>,
    /// Allowed file extensions.
    #[serde(default)]
    pub(super) ext_filter: Vec<String>,
    /// Maximum number of visible rows.
    #[serde(default)]
    pub(super) max_visible: Option<usize>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
    /// Whether the field is required.
    #[serde(default)]
    pub(super) required: Option<bool>,
    /// Validation rules applied to the value.
    #[serde(default)]
    pub(super) validators: Vec<ValidatorDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TreeViewDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Tree nodes with item and depth metadata.
    pub(super) nodes: Vec<TreeNodeDef>,
    /// Maximum number of visible rows.
    #[serde(default)]
    pub(super) max_visible: Option<usize>,
    /// Whether to render the label.
    #[serde(default)]
    pub(super) show_label: Option<bool>,
    /// Whether to render indentation guides.
    #[serde(default)]
    pub(super) indent_guides: Option<bool>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct ObjectEditorDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Initial object value.
    #[serde(default)]
    #[schemars(schema_with = "super::doc_model::yaml_value_schema")]
    pub(super) value: Option<serde_yaml::Value>,
    /// Maximum number of visible rows.
    #[serde(default)]
    pub(super) max_visible: Option<usize>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct SnippetDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Snippet template string.
    pub(super) template: String,
    /// Nested interactive widget definitions.
    #[serde(default)]
    pub(super) inputs: Vec<WidgetDef>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TableDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Table rendering style.
    #[serde(default)]
    pub(super) style: Option<String>,
    /// Whether to show row numbers.
    #[serde(default)]
    pub(super) row_numbers: Option<bool>,
    /// Initial number of rows.
    #[serde(default)]
    pub(super) initial_rows: Option<usize>,
    /// Column definitions with embedded widgets.
    pub(super) columns: Vec<TableColumnDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct RepeaterDef {
    /// Unique widget identifier within the step.
    pub(super) id: String,
    /// Visible widget label.
    pub(super) label: String,
    /// Repeater layout mode.
    #[serde(default)]
    pub(super) layout: Option<String>,
    /// Whether to render the label.
    #[serde(default)]
    pub(super) show_label: Option<bool>,
    /// Whether to show progress for items.
    #[serde(default)]
    pub(super) show_progress: Option<bool>,
    /// Optional header template for each item.
    #[serde(default)]
    pub(super) header_template: Option<String>,
    /// Relative path used as item label.
    #[serde(default)]
    pub(super) item_label_path: Option<String>,
    /// Initial item values.
    #[serde(default)]
    #[schemars(schema_with = "super::doc_model::yaml_value_schema")]
    pub(super) items: Vec<serde_yaml::Value>,
    /// Store selector updated on submit.
    #[serde(default)]
    pub(super) submit_target: Option<String>,
    /// Repeater field definitions with embedded widgets.
    pub(super) fields: Vec<RepeaterFieldDef>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum SelectListOptionDef {
    Plain(String),
    Detailed {
        /// Stored option value.
        value: String,
        /// Visible option title.
        title: String,
        /// Detailed description shown in the list.
        description: String,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TableColumnDef {
    /// Visible column header.
    pub(super) header: String,
    /// Embedded widget used by the column.
    pub(super) widget: EmbeddedWidgetDef,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct RepeaterFieldDef {
    /// Stable field key used in item values.
    pub(super) key: String,
    /// Visible field label.
    pub(super) label: String,
    /// Embedded widget used by the field.
    pub(super) widget: EmbeddedWidgetDef,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum EmbeddedWidgetDef {
    TextInput(EmbeddedTextInputDef),
    MaskedInput(EmbeddedMaskedInputDef),
    Select(EmbeddedSelectDef),
    Slider(EmbeddedSliderDef),
    Checkbox(EmbeddedCheckboxDef),
    ArrayInput(EmbeddedArrayInputDef),
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedTextInputDef {
    /// Placeholder text shown when the field is empty.
    #[serde(default)]
    pub(super) placeholder: Option<String>,
    /// Text display mode.
    #[serde(default)]
    pub(super) mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedMaskedInputDef {
    /// Mask pattern.
    pub(super) mask: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedSelectDef {
    /// Available option values.
    pub(super) options: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedSliderDef {
    /// Minimum value.
    pub(super) min: i64,
    /// Maximum value.
    pub(super) max: i64,
    /// Increment step.
    #[serde(default)]
    pub(super) step: Option<i64>,
    /// Optional display unit.
    #[serde(default)]
    pub(super) unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedCheckboxDef {
    /// Initial checked state.
    #[serde(default)]
    pub(super) checked: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct EmbeddedArrayInputDef {
    /// Initial string items.
    #[serde(default)]
    pub(super) items: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TreeNodeDef {
    /// Rendered item label.
    pub(super) item: String,
    /// Indentation depth for the node.
    pub(super) depth: usize,
    /// Whether the node has children.
    pub(super) has_children: bool,
    /// Whether the node starts expanded.
    #[serde(default)]
    pub(super) expanded: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct TaskLogStepDef {
    /// Visible step label.
    pub(super) label: String,
    /// Referenced task id.
    pub(super) task_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ConfirmModeDef {
    Relaxed,
    Strict {
        /// Exact confirmation word required in strict mode.
        word: String,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ValidatorDef {
    Required {
        /// Custom validation error message.
        #[serde(default)]
        message: Option<String>,
    },
    MinLength {
        /// Minimum text length.
        value: usize,
    },
    MaxLength {
        /// Maximum text length.
        value: usize,
    },
    MinSelections {
        /// Minimum number of selected items.
        value: usize,
    },
    MaxSelections {
        /// Maximum number of selected items.
        value: usize,
    },
    MustBeChecked,
    MinValue {
        /// Minimum numeric value.
        value: f64,
    },
    MaxValue {
        /// Maximum numeric value.
        value: f64,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ProgressTransitionDef {
    Immediate,
    Tween {
        /// Duration of the tween in milliseconds.
        duration_ms: u64,
        /// Optional easing function name.
        #[serde(default)]
        easing: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(super) struct CommandRunnerCommandDef {
    /// Visible command label.
    pub(super) label: String,
    /// Executed program name.
    pub(super) program: String,
    /// Program arguments.
    #[serde(default)]
    pub(super) args: Vec<String>,
}
