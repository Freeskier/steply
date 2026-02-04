use crate::core::node::{Node, NodeId};
use crate::core::node_registry::NodeRegistry;
use crate::core::step::Step;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Active,
    Done,
    Cancelled,
}

pub struct Flow {
    steps: Vec<Step>,
    registry: NodeRegistry,
    current: usize,
    statuses: Vec<StepStatus>,
}

impl Flow {
    pub fn new(steps_with_nodes: Vec<(Step, Vec<(NodeId, Node)>)>) -> Self {
        let mut registry = NodeRegistry::new();
        let mut steps = Vec::with_capacity(steps_with_nodes.len());

        for (step, nodes) in steps_with_nodes {
            registry.extend(nodes);
            steps.push(step);
        }

        let mut statuses = vec![StepStatus::Pending; steps.len()];
        if !statuses.is_empty() {
            statuses[0] = StepStatus::Active;
        }

        Self {
            steps,
            registry,
            current: 0,
            statuses,
        }
    }

    pub fn registry(&self) -> &NodeRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut NodeRegistry {
        &mut self.registry
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn current_step(&self) -> &Step {
        &self.steps[self.current]
    }

    pub fn current_step_mut(&mut self) -> &mut Step {
        &mut self.steps[self.current]
    }

    pub fn step_at(&self, index: usize) -> Option<&Step> {
        self.steps.get(index)
    }

    pub fn current_status(&self) -> StepStatus {
        self.statuses
            .get(self.current)
            .copied()
            .unwrap_or(StepStatus::Active)
    }

    pub fn status_at(&self, index: usize) -> StepStatus {
        self.statuses
            .get(index)
            .copied()
            .unwrap_or(StepStatus::Pending)
    }

    pub fn has_next(&self) -> bool {
        self.current + 1 < self.steps.len()
    }

    pub fn advance(&mut self) {
        if !self.has_next() {
            return;
        }

        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Done;
        }
        self.current += 1;
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Active;
        }
    }

    pub fn cancel_current(&mut self) {
        if let Some(status) = self.statuses.get_mut(self.current) {
            *status = StepStatus::Cancelled;
        }
    }
}
