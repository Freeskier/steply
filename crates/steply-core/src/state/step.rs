use crate::core::store_refs::{parse_store_selector, template_expressions};
use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValueTarget};
use crate::state::change::StoreCommitPolicy;
use crate::state::store::ValueStore;
use crate::state::validation::{StepContext, StepIssue, StepValidator};
use crate::widgets::node::{Component, Node, NodeWalkScope, walk_nodes};
use crate::widgets::shared::binding::{ReadBinding, StoreBinding};
use crate::widgets::traits::{InteractiveNode, OutputNode};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Running,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepNavigation {
    #[default]
    Allowed,

    Locked,

    Reset,

    Destructive {
        warning: String,
    },
}

pub struct Step {
    pub id: String,
    pub prompt: String,
    pub description: Option<String>,
    pub nodes: Vec<Node>,
    pub binding_plan: StepBindingPlan,
    pub validators: Vec<StepValidator>,
    pub navigation: StepNavigation,
    pub when: Option<StepCondition>,
}

#[derive(Debug, Clone, Default)]
pub struct StepBindingPlan {
    pub direct_value_nodes: Vec<StepDirectValueBinding>,
    pub derived_writers: Vec<StepDerivedBindingWriter>,
}

#[derive(Debug, Clone)]
pub struct StepDirectValueBinding {
    pub node_id: String,
    pub target: ValueTarget,
    pub commit_policy: StoreCommitPolicy,
}

#[derive(Debug, Clone)]
pub struct StepDerivedBindingWriter {
    pub node_id: String,
    pub write_targets: Vec<ValueTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepCondition {
    Equal { field: String, value: Value },
    NotEqual { field: String, value: Value },
    NotEmpty { field: String },
    All(Vec<StepCondition>),
    Any(Vec<StepCondition>),
    Not(Box<StepCondition>),
}

impl StepCondition {
    pub fn evaluate(&self, store: &ValueStore) -> bool {
        match self {
            Self::Equal { field, value } => store
                .get_selector(field.as_str())
                .is_some_and(|v| v == value),
            Self::NotEqual { field, value } => store.get_selector(field.as_str()) != Some(value),
            Self::NotEmpty { field } => store
                .get_selector(field.as_str())
                .is_some_and(|v| !v.is_empty()),
            Self::All(conditions) => conditions.iter().all(|condition| condition.evaluate(store)),
            Self::Any(conditions) => conditions.iter().any(|condition| condition.evaluate(store)),
            Self::Not(condition) => !condition.evaluate(store),
        }
    }
}

impl Step {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, nodes: Vec<Node>) -> Self {
        let binding_plan = StepBindingPlan::from_nodes(nodes.as_slice());
        Self {
            id: id.into(),
            prompt: prompt.into(),
            description: None,
            nodes,
            binding_plan,
            validators: Vec::new(),
            navigation: StepNavigation::default(),
            when: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_validator(mut self, validator: StepValidator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn require(mut self, field_id: impl Into<String>, message: impl Into<String>) -> Self {
        self.validators
            .push(required_validator(field_id.into(), message.into()));
        self
    }

    pub fn warn_if_empty(
        mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.validators
            .push(warning_if_empty_validator(field_id.into(), message.into()));
        self
    }

    pub fn validate(
        mut self,
        f: impl Fn(&StepContext) -> Option<StepIssue> + Send + Sync + 'static,
    ) -> Self {
        self.validators.push(Box::new(f));
        self
    }

    pub fn with_navigation(mut self, navigation: StepNavigation) -> Self {
        self.navigation = navigation;
        self
    }

    pub fn with_when(mut self, when: StepCondition) -> Self {
        self.when = Some(when);
        self
    }

    pub fn is_visible(&self, store: &ValueStore) -> bool {
        self.when
            .as_ref()
            .is_none_or(|condition| condition.evaluate(store))
    }

    pub fn builder(id: impl Into<String>, prompt: impl Into<String>) -> StepBuilder {
        StepBuilder::new(id, prompt)
    }
}

pub struct StepBuilder {
    id: String,
    prompt: String,
    description: Option<String>,
    nodes: Vec<Node>,
    validators: Vec<StepValidator>,
    navigation: StepNavigation,
    when: Option<StepCondition>,
}

impl StepBuilder {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            description: None,
            nodes: Vec::new(),
            validators: Vec::new(),
            navigation: StepNavigation::default(),
            when: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.nodes.extend(nodes);
        self
    }

    pub fn input(mut self, input: impl InteractiveNode + 'static) -> Self {
        self.nodes.push(Node::Input(Box::new(input)));
        self
    }

    pub fn component(mut self, component: impl Component + 'static) -> Self {
        self.nodes.push(Node::Component(Box::new(component)));
        self
    }

    pub fn output(mut self, output: impl OutputNode + 'static) -> Self {
        self.nodes.push(Node::Output(Box::new(output)));
        self
    }

    pub fn validator(mut self, validator: StepValidator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn require(mut self, field_id: impl Into<String>, message: impl Into<String>) -> Self {
        self.validators
            .push(required_validator(field_id.into(), message.into()));
        self
    }

    pub fn warn_if_empty(
        mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.validators
            .push(warning_if_empty_validator(field_id.into(), message.into()));
        self
    }

    pub fn validate(
        mut self,
        f: impl Fn(&StepContext) -> Option<StepIssue> + Send + Sync + 'static,
    ) -> Self {
        self.validators.push(Box::new(f));
        self
    }

    pub fn navigation(mut self, navigation: StepNavigation) -> Self {
        self.navigation = navigation;
        self
    }

    pub fn when(mut self, when: StepCondition) -> Self {
        self.when = Some(when);
        self
    }

    pub fn build(self) -> Step {
        let binding_plan = StepBindingPlan::from_nodes(self.nodes.as_slice());
        Step {
            id: self.id,
            prompt: self.prompt,
            description: self.description,
            nodes: self.nodes,
            binding_plan,
            validators: self.validators,
            navigation: self.navigation,
            when: self.when,
        }
    }
}

#[derive(Clone)]
struct BindingNodeInfo {
    node_id: String,
    direct_value_target: Option<ValueTarget>,
    read_selectors: Vec<ValueTarget>,
    write_targets: Vec<ValueTarget>,
    derived_writer: bool,
    commit_policy: StoreCommitPolicy,
}

impl StepBindingPlan {
    fn from_nodes(nodes: &[Node]) -> Self {
        let infos = collect_binding_node_infos(nodes);
        let direct_value_nodes = infos
            .iter()
            .filter_map(|info| {
                Some(StepDirectValueBinding {
                    node_id: info.node_id.clone(),
                    target: info.direct_value_target.clone()?,
                    commit_policy: info.commit_policy,
                })
            })
            .collect();
        let derived_writers = topological_derived_writers(infos.as_slice());
        Self {
            direct_value_nodes,
            derived_writers,
        }
    }
}

fn collect_binding_node_infos(nodes: &[Node]) -> Vec<BindingNodeInfo> {
    let mut infos = Vec::new();
    walk_nodes(nodes, NodeWalkScope::Recursive, &mut |node| {
        let Some(binding) = node.store_binding() else {
            return;
        };
        infos.push(BindingNodeInfo {
            node_id: node.id().to_string(),
            direct_value_target: binding.value.clone(),
            read_selectors: binding_read_selectors(binding),
            write_targets: binding
                .writes
                .iter()
                .map(|binding| binding.target.clone())
                .collect(),
            derived_writer: binding.value.is_none()
                && binding.reads.is_some()
                && !binding.writes.is_empty(),
            commit_policy: node.commit_policy(),
        });
    });
    infos
}

fn binding_read_selectors(binding: &StoreBinding) -> Vec<ValueTarget> {
    let mut selectors = Vec::new();
    if let Some(options) = &binding.options {
        collect_read_binding_selectors(options, &mut selectors);
    }
    if let Some(reads) = &binding.reads {
        collect_read_binding_selectors(reads, &mut selectors);
    }
    selectors
}

fn collect_read_binding_selectors(binding: &ReadBinding, out: &mut Vec<ValueTarget>) {
    match binding {
        ReadBinding::Selector(target) => out.push(target.clone()),
        ReadBinding::Literal(_) => {}
        ReadBinding::Template(template) => {
            for expr in template_expressions(template) {
                if let Ok(target) = parse_store_selector(expr.as_str()) {
                    out.push(target);
                }
            }
        }
        ReadBinding::Object(entries) => {
            for binding in entries.values() {
                collect_read_binding_selectors(binding, out);
            }
        }
        ReadBinding::List(items) => {
            for item in items {
                collect_read_binding_selectors(item, out);
            }
        }
    }
}

fn topological_derived_writers(infos: &[BindingNodeInfo]) -> Vec<StepDerivedBindingWriter> {
    let derived = infos
        .iter()
        .enumerate()
        .filter(|(_, info)| info.derived_writer)
        .collect::<Vec<_>>();
    if derived.is_empty() {
        return Vec::new();
    }

    let mut indegree = vec![0usize; derived.len()];
    let mut edges = vec![Vec::<usize>::new(); derived.len()];

    for (target_index, (_, target_info)) in derived.iter().enumerate() {
        for (source_index, (_, source_info)) in derived.iter().enumerate() {
            if source_index == target_index {
                continue;
            }
            if source_info.write_targets.iter().any(|write| {
                target_info
                    .read_selectors
                    .iter()
                    .any(|read| target_affects_selector(write, read))
            }) {
                edges[source_index].push(target_index);
                indegree[target_index] += 1;
            }
        }
    }

    let mut ready = VecDeque::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.push_back(index);
        }
    }

    let mut ordered = Vec::with_capacity(derived.len());
    while let Some(index) = ready.pop_front() {
        ordered.push(StepDerivedBindingWriter {
            node_id: derived[index].1.node_id.clone(),
            write_targets: derived[index].1.write_targets.clone(),
        });
        let mut next = edges[index].clone();
        next.sort_unstable();
        next.dedup();
        for target in next {
            indegree[target] = indegree[target].saturating_sub(1);
            if indegree[target] == 0 {
                ready.push_back(target);
            }
        }
    }

    if ordered.len() == derived.len() {
        return ordered;
    }

    let mut fallback = Vec::with_capacity(derived.len());
    let mut seen = HashSet::<String>::new();
    for (_, info) in derived {
        if seen.insert(info.node_id.clone()) {
            fallback.push(StepDerivedBindingWriter {
                node_id: info.node_id.clone(),
                write_targets: info.write_targets.clone(),
            });
        }
    }
    fallback
}

fn target_affects_selector(write: &ValueTarget, read: &ValueTarget) -> bool {
    if write.root() != read.root() {
        return false;
    }
    let write_path = write
        .nested_path()
        .map(|path| path.segments())
        .unwrap_or(&[]);
    let read_path = read
        .nested_path()
        .map(|path| path.segments())
        .unwrap_or(&[]);

    path_is_prefix(write_path, read_path) || path_is_prefix(read_path, write_path)
}

fn path_is_prefix(prefix: &[PathSegment], full: &[PathSegment]) -> bool {
    prefix.len() <= full.len() && prefix.iter().zip(full.iter()).all(|(a, b)| a == b)
}

fn required_validator(field_id: String, message: String) -> StepValidator {
    Box::new(move |ctx: &StepContext| {
        if ctx.is_empty(&field_id) {
            Some(StepIssue::error(&message))
        } else {
            None
        }
    })
}

fn warning_if_empty_validator(field_id: String, message: String) -> StepValidator {
    Box::new(move |ctx: &StepContext| {
        if ctx.is_empty(&field_id) {
            Some(StepIssue::warning(&message))
        } else {
            None
        }
    })
}
