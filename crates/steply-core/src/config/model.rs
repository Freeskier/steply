use serde::Deserialize;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum NavigationDef {
    Allowed,
    Locked,
    Reset,
    Destructive { warning: String },
}

#[derive(Debug, Deserialize)]
pub(super) struct FlowItemDef {
    pub(super) step: String,
    #[serde(default)]
    pub(super) when: Option<WhenDef>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub(super) struct SubscriptionDef {
    pub(super) task: String,
    pub(super) trigger: TriggerDef,
    #[serde(default)]
    pub(super) target: Option<String>,
    #[serde(default)]
    pub(super) enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TriggerDef {
    #[serde(default)]
    pub(super) on_input: Option<OnInputTriggerDef>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OnInputTriggerDef {
    #[serde(rename = "ref")]
    pub(super) field_ref: String,
    #[serde(default)]
    pub(super) debounce_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct WhenDef {
    #[serde(default, rename = "ref")]
    pub(super) field_ref: Option<String>,
    #[serde(default)]
    pub(super) equal: Option<serde_yaml::Value>,
    #[serde(default)]
    pub(super) not_equal: Option<serde_yaml::Value>,
    #[serde(default)]
    pub(super) not_empty: Option<bool>,
    #[serde(default)]
    pub(super) all: Vec<WhenDef>,
    #[serde(default)]
    pub(super) any: Vec<WhenDef>,
    #[serde(default)]
    pub(super) not: Option<Box<WhenDef>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum WidgetDef {
    TextOutput {
        id: String,
        text: String,
    },
    UrlOutput {
        id: String,
        url: String,
        #[serde(default)]
        name: Option<String>,
    },
    ThinkingOutput {
        id: String,
        label: String,
        text: String,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        tail_len: Option<usize>,
        #[serde(default)]
        tick_ms: Option<u64>,
        #[serde(default)]
        base_rgb: Option<[u8; 3]>,
        #[serde(default)]
        peak_rgb: Option<[u8; 3]>,
    },
    ProgressOutput {
        id: String,
        label: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        bar_width: Option<usize>,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        transition: Option<ProgressTransitionDef>,
    },
    ChartOutput {
        id: String,
        label: String,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        capacity: Option<usize>,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        gradient: Option<bool>,
    },
    TableOutput {
        id: String,
        label: String,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        headers: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    DiffOutput {
        id: String,
        label: String,
        old: String,
        new: String,
        #[serde(default)]
        max_visible: Option<usize>,
    },
    TaskLogOutput {
        id: String,
        #[serde(default)]
        visible_lines: Option<usize>,
        #[serde(default)]
        spinner_style: Option<String>,
        steps: Vec<TaskLogStepDef>,
    },
    TextInput {
        id: String,
        label: String,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        completion_items: Vec<String>,
        #[serde(default)]
        submit_target: Option<String>,
        #[serde(default)]
        change_targets: Vec<String>,
    },
    ArrayInput {
        id: String,
        label: String,
        #[serde(default)]
        items: Vec<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
    },
    ButtonInput {
        id: String,
        label: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        task_id: Option<String>,
    },
    Select {
        id: String,
        label: String,
        options: Vec<String>,
        #[serde(default)]
        selected: Option<usize>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    ChoiceInput {
        id: String,
        label: String,
        options: Vec<String>,
        #[serde(default)]
        bullets: Option<bool>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    SelectList {
        id: String,
        label: String,
        #[serde(default)]
        options: Vec<SelectListOptionDef>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        max_visible: Option<usize>,
        #[serde(default)]
        selected: Vec<usize>,
        #[serde(default)]
        show_label: Option<bool>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    MaskedInput {
        id: String,
        label: String,
        mask: String,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    Slider {
        id: String,
        label: String,
        min: i64,
        max: i64,
        #[serde(default)]
        step: Option<i64>,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        track_len: Option<usize>,
        #[serde(default)]
        default: Option<f64>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        change_targets: Vec<String>,
    },
    ColorInput {
        id: String,
        label: String,
        #[serde(default)]
        rgb: Option<[u8; 3]>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    ConfirmInput {
        id: String,
        label: String,
        #[serde(default)]
        mode: Option<ConfirmModeDef>,
        #[serde(default)]
        yes_label: Option<String>,
        #[serde(default)]
        no_label: Option<String>,
        #[serde(default)]
        default: Option<bool>,
    },
    Checkbox {
        id: String,
        label: String,
        #[serde(default)]
        checked: Option<bool>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
    },
    Calendar {
        id: String,
        label: String,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    Textarea {
        id: String,
        #[serde(default)]
        min_height: Option<usize>,
        #[serde(default)]
        max_height: Option<usize>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
    },
    CommandRunner {
        id: String,
        label: String,
        #[serde(default)]
        run_mode: Option<String>,
        #[serde(default)]
        on_error: Option<String>,
        #[serde(default)]
        advance_on_success: Option<bool>,
        #[serde(default)]
        visible_lines: Option<usize>,
        #[serde(default)]
        spinner_style: Option<String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
        commands: Vec<CommandRunnerCommandDef>,
    },
    FileBrowser {
        id: String,
        label: String,
        #[serde(default)]
        browser_mode: Option<String>,
        #[serde(default)]
        display_mode: Option<String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        recursive: Option<bool>,
        #[serde(default)]
        hide_hidden: Option<bool>,
        #[serde(default)]
        ext_filter: Vec<String>,
        #[serde(default)]
        max_visible: Option<usize>,
        #[serde(default)]
        submit_target: Option<String>,
        #[serde(default)]
        required: Option<bool>,
        #[serde(default)]
        validators: Vec<ValidatorDef>,
    },
    TreeView {
        id: String,
        label: String,
        nodes: Vec<TreeNodeDef>,
        #[serde(default)]
        max_visible: Option<usize>,
        #[serde(default)]
        show_label: Option<bool>,
        #[serde(default)]
        indent_guides: Option<bool>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    ObjectEditor {
        id: String,
        label: String,
        #[serde(default)]
        value: Option<serde_yaml::Value>,
        #[serde(default)]
        max_visible: Option<usize>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    Snippet {
        id: String,
        label: String,
        template: String,
        #[serde(default)]
        inputs: Vec<WidgetDef>,
        #[serde(default)]
        submit_target: Option<String>,
    },
    Table {
        id: String,
        label: String,
        #[serde(default)]
        style: Option<String>,
        #[serde(default)]
        row_numbers: Option<bool>,
        #[serde(default)]
        initial_rows: Option<usize>,
        columns: Vec<TableColumnDef>,
    },
    Repeater {
        id: String,
        label: String,
        #[serde(default)]
        layout: Option<String>,
        #[serde(default)]
        show_label: Option<bool>,
        #[serde(default)]
        show_progress: Option<bool>,
        #[serde(default)]
        header_template: Option<String>,
        #[serde(default)]
        item_label_path: Option<String>,
        #[serde(default)]
        items: Vec<serde_yaml::Value>,
        #[serde(default)]
        submit_target: Option<String>,
        fields: Vec<RepeaterFieldDef>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum SelectListOptionDef {
    Plain(String),
    Detailed {
        value: String,
        title: String,
        description: String,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct TableColumnDef {
    pub(super) header: String,
    pub(super) widget: EmbeddedWidgetDef,
}

#[derive(Debug, Deserialize)]
pub(super) struct RepeaterFieldDef {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) widget: EmbeddedWidgetDef,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum EmbeddedWidgetDef {
    TextInput {
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default)]
        mode: Option<String>,
    },
    MaskedInput {
        mask: String,
    },
    Select {
        options: Vec<String>,
    },
    Slider {
        min: i64,
        max: i64,
        #[serde(default)]
        step: Option<i64>,
        #[serde(default)]
        unit: Option<String>,
    },
    Checkbox {
        #[serde(default)]
        checked: Option<bool>,
    },
    ArrayInput {
        #[serde(default)]
        items: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct TreeNodeDef {
    pub(super) item: String,
    pub(super) depth: usize,
    pub(super) has_children: bool,
    #[serde(default)]
    pub(super) expanded: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TaskLogStepDef {
    pub(super) label: String,
    pub(super) task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ConfirmModeDef {
    Relaxed,
    Strict { word: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ValidatorDef {
    Required {
        #[serde(default)]
        message: Option<String>,
    },
    MinLength {
        value: usize,
    },
    MaxLength {
        value: usize,
    },
    MinSelections {
        value: usize,
    },
    MaxSelections {
        value: usize,
    },
    MustBeChecked,
    MinValue {
        value: f64,
    },
    MaxValue {
        value: f64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ProgressTransitionDef {
    Immediate,
    Tween {
        duration_ms: u64,
        #[serde(default)]
        easing: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct CommandRunnerCommandDef {
    pub(super) label: String,
    pub(super) program: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
}
