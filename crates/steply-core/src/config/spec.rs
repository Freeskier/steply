use super::model::{NavigationDef, WhenDef, WidgetDef};

#[derive(Debug)]
pub(super) struct ConfigSpec {
    pub steps: Vec<StepSpec>,
    pub tasks: Vec<TaskTemplateSpec>,
    pub subscriptions: Vec<SubscriptionSpec>,
}

#[derive(Debug)]
pub(super) struct StepSpec {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub navigation: Option<NavigationDef>,
    pub when: Option<WhenDef>,
    pub widgets: Vec<WidgetDef>,
}

#[derive(Debug, Clone)]
pub(super) struct TaskTemplateSpec {
    pub id: String,
    pub kind: String,
    pub program: String,
    pub args: Vec<String>,
    pub parse: Option<String>,
    pub timeout_ms: Option<u64>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SubscriptionSpec {
    pub task: String,
    pub trigger: SubscriptionTriggerSpec,
    pub target: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(super) enum SubscriptionTriggerSpec {
    OnInput { field_ref: String, debounce_ms: u64 },
}
