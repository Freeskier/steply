use super::model::{NavigationDef, WhenDef, WidgetDef, WriteBindingDef};
use crate::task::TaskTrigger;

#[derive(Debug)]
pub(super) struct ConfigSpec {
    pub steps: Vec<StepSpec>,
    pub tasks: Vec<TaskTemplateSpec>,
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
    pub reads: Option<serde_yaml::Value>,
    pub timeout_ms: Option<u64>,
    pub enabled: bool,
    pub triggers: Vec<TaskTrigger>,
    pub writes: Option<WriteBindingDef>,
}
