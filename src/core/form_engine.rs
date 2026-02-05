use crate::core::component::EventContext;
use crate::core::form_event::FormEvent;
use crate::core::node::NodeId;
use crate::core::node::{Node, find_input_mut};
use crate::core::validation;
use crate::core::value::Value;
use crate::core::widget::Widget;
use crate::inputs::{Input, InputError};
use crate::terminal::KeyEvent;

#[derive(Clone, Debug)]
struct FocusTarget {
    path: NodePath,
    id: NodeId,
    kind: FocusKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FocusKind {
    Input,
    Component,
}

pub type NodePath = Vec<usize>;

#[derive(Debug, Clone)]
pub struct ComponentValue {
    pub id: NodeId,
    pub value: Value,
}

#[derive(Debug, Default, Clone)]
pub struct EngineOutput {
    pub events: Vec<FormEvent>,
    pub produced: Vec<ComponentValue>,
    pub handled: bool,
}

pub struct FormEngine {
    input_ids: Vec<NodeId>,
    focus_targets: Vec<FocusTarget>,
    focus_index: Option<usize>,
}

impl FormEngine {
    pub fn from_nodes(nodes: &mut [Node]) -> Self {
        let input_ids = collect_input_ids(nodes);
        let focus_targets = collect_focus_targets(nodes, &[]);
        let mut engine = Self {
            input_ids,
            focus_targets,
            focus_index: None,
        };

        if !engine.focus_targets.is_empty() {
            engine.set_focus_internal(nodes, Some(0));
        }

        engine
    }

    pub fn reset_with_nodes(&mut self, nodes: &mut [Node]) {
        self.input_ids = collect_input_ids(nodes);
        self.focus_targets = collect_focus_targets(nodes, &[]);
        self.focus_index = None;

        if !self.focus_targets.is_empty() {
            self.set_focus_internal(nodes, Some(0));
        }
    }

    pub fn focus_index(&self) -> Option<usize> {
        self.focus_index
    }

    pub fn focused_target(&self) -> Option<&NodeId> {
        self.focus_index
            .and_then(|i| self.focus_targets.get(i))
            .map(|target| &target.id)
    }

    pub fn focused_node_id(&self) -> Option<&NodeId> {
        self.focused_target()
    }

    pub fn handle_tab_completion(&mut self, nodes: &mut [Node]) -> bool {
        let Some(input) = self.focused_input_mut(nodes) else {
            return false;
        };

        if !input.supports_tab_completion() {
            return false;
        }

        input.handle_tab_completion()
    }

    pub fn move_focus(&mut self, nodes: &mut [Node], direction: isize) -> Vec<FormEvent> {
        if self.focus_targets.is_empty() {
            return vec![];
        }

        let mut events = Vec::new();
        if let Some(input) = self.focused_input_mut(nodes) {
            if let Err(err) = validation::validate_input(input) {
                input.set_error(Some(InputError::hidden(err)));
            } else {
                input.clear_error();
            }
        }

        let current = self.focus_index.unwrap_or(0);
        let len = self.focus_targets.len() as isize;
        let next = ((current as isize + direction + len) % len) as usize;

        self.set_focus(nodes, Some(next), &mut events);
        events
    }

    pub fn set_focus(
        &mut self,
        nodes: &mut [Node],
        new_index: Option<usize>,
        events: &mut Vec<FormEvent>,
    ) {
        let from_target = self.focus_index.and_then(|i| self.focus_targets.get(i));
        let to_target = new_index.and_then(|i| self.focus_targets.get(i));

        if from_target.map(|t| &t.id) == to_target.map(|t| &t.id) {
            return;
        }

        if let Some(target) = from_target {
            self.set_target_focus(nodes, target, false);
        }

        if let Some(target) = to_target {
            self.set_target_focus(nodes, target, true);
        }

        self.focus_index = new_index;
        let from_id = from_target.map(|t| t.id.clone());
        let to_id = to_target.map(|t| t.id.clone());
        events.push(FormEvent::FocusChanged {
            from: from_id,
            to: to_id,
        });
    }

    pub fn clear_focus(&mut self, nodes: &mut [Node]) {
        if let Some(target) = self.focus_index.and_then(|i| self.focus_targets.get(i)) {
            self.set_target_focus(nodes, target, false);
        }
        self.focus_index = None;
    }

    pub fn find_index_by_id(&self, id: &str) -> Option<usize> {
        self.focus_targets.iter().position(|target| target.id == id)
    }

    pub fn handle_key(&mut self, nodes: &mut [Node], key: KeyEvent) -> EngineOutput {
        let mut output = EngineOutput::default();

        let Some(target) = self
            .focus_index
            .and_then(|i| self.focus_targets.get(i))
            .cloned()
        else {
            return output;
        };

        let Some(node) = node_at_path_mut(nodes, &target.path) else {
            return output;
        };

        let Some(mut widget) = node.widget_ref_mut() else {
            return output;
        };

        let mut ctx = EventContext::new();
        let handled = widget.handle_key(key.code, key.modifiers, &mut ctx);
        let response = ctx.into_response(handled);
        output.handled = response.handled;

        if let Some(value) = response.produced {
            output.produced.push(ComponentValue {
                id: target.id.clone(),
                value,
            });
        }

        for change in response.changes {
            if let Some(input) = find_input_mut(nodes, &change.id) {
                let events =
                    self.apply_input_change(input, &change.id, &change.value, change.apply);
                output.events.extend(events);
            }
        }

        if response.submit_requested {
            output.events.push(FormEvent::SubmitRequested);
        }

        output
    }

    pub fn handle_delete_word(&mut self, nodes: &mut [Node], forward: bool) -> Vec<FormEvent> {
        let Some(target) = self
            .focus_index
            .and_then(|i| self.focus_targets.get(i))
            .cloned()
        else {
            return vec![];
        };

        let Some(node) = node_at_path_mut(nodes, &target.path) else {
            return vec![];
        };

        let Some(widget) = node.widget_ref_mut() else {
            return vec![];
        };

        let mut ctx = EventContext::new();
        let handled = match widget {
            crate::core::node::WidgetRefMut::Input(input) => {
                if forward {
                    input.delete_word_forward();
                } else {
                    input.delete_word();
                }
                ctx.record_input(input.id().to_string(), input.value());
                ctx.handled();
                true
            }
            crate::core::node::WidgetRefMut::Component(component) => {
                if forward {
                    component.delete_word_forward(&mut ctx)
                } else {
                    component.delete_word(&mut ctx)
                }
            }
        };

        if !handled {
            return vec![];
        }

        let response = ctx.into_response(handled);
        let mut events = Vec::new();

        for change in response.changes {
            if let Some(input) = find_input_mut(nodes, &change.id) {
                let change_events =
                    self.apply_input_change(input, &change.id, &change.value, change.apply);
                events.extend(change_events);
            }
        }

        if response.submit_requested {
            events.push(FormEvent::SubmitRequested);
        }

        events
    }

    pub fn validate_focused(&self, nodes: &mut [Node]) -> Result<(), (NodeId, String)> {
        let Some(target) = self.focus_index.and_then(|i| self.focus_targets.get(i)) else {
            return Ok(());
        };

        if target.kind != FocusKind::Input {
            return Ok(());
        }

        let Some(input) = find_input_mut(nodes, &target.id) else {
            return Ok(());
        };

        match validation::validate_input(input) {
            Ok(()) => {
                input.clear_error();
                Ok(())
            }
            Err(err) => {
                input.set_error(Some(InputError::inline(&err)));
                Err((target.id.clone(), err))
            }
        }
    }

    pub fn apply_errors(&mut self, nodes: &mut [Node], errors: &[(NodeId, String)]) -> Vec<NodeId> {
        let mut scheduled = Vec::new();

        for id in &self.input_ids {
            let Some(input) = find_input_mut(nodes, id) else {
                continue;
            };

            if let Some((_, err)) = errors.iter().find(|(eid, _)| eid == id) {
                input.set_error(Some(InputError::inline(err)));
                scheduled.push(id.clone());
            } else {
                input.clear_error();
            }
        }

        scheduled
    }

    pub fn clear_error(&self, nodes: &mut [Node], id: &str) {
        if let Some(input) = find_input_mut(nodes, id) {
            input.clear_error();
        }
    }

    pub fn advance_focus(&mut self, nodes: &mut [Node], events: &mut Vec<FormEvent>) -> bool {
        let Some(current) = self.focus_index else {
            return false;
        };

        let next = current + 1;
        if next < self.focus_targets.len() {
            self.set_focus(nodes, Some(next), events);
            true
        } else {
            false
        }
    }

    pub fn input_ids(&self) -> &[NodeId] {
        &self.input_ids
    }

    fn set_focus_internal(&mut self, nodes: &mut [Node], new_index: Option<usize>) {
        if let Some(target) = self.focus_index.and_then(|i| self.focus_targets.get(i)) {
            self.set_target_focus(nodes, target, false);
        }

        if let Some(idx) = new_index {
            if let Some(target) = self.focus_targets.get(idx) {
                self.set_target_focus(nodes, target, true);
            }
        }

        self.focus_index = new_index;
    }

    fn focused_input_mut<'a>(&self, nodes: &'a mut [Node]) -> Option<&'a mut dyn Input> {
        let target = self.focus_index.and_then(|i| self.focus_targets.get(i))?;
        if target.kind != FocusKind::Input {
            return None;
        }
        find_input_mut(nodes, &target.id)
    }

    fn apply_input_change(
        &self,
        input: &mut dyn Input,
        id: &str,
        value: &str,
        apply: bool,
    ) -> Vec<FormEvent> {
        if apply {
            input.set_value(value.to_string());
        }
        let mut events = Vec::new();
        events.push(FormEvent::InputChanged {
            id: id.to_string(),
            value: value.to_string(),
        });
        events.push(FormEvent::ErrorCancelled { id: id.to_string() });
        input.clear_error();

        if let Err(err) = validation::validate_input(input) {
            input.set_error(Some(InputError::hidden(err)));
        }

        events
    }

    fn set_target_focus(&self, nodes: &mut [Node], target: &FocusTarget, focused: bool) {
        let Some(node) = node_at_path_mut(nodes, &target.path) else {
            return;
        };
        node.set_focused(focused);
    }
}

fn collect_input_ids(nodes: &[Node]) -> Vec<NodeId> {
    let mut ids = Vec::new();
    collect_input_ids_inner(nodes, &mut ids);
    ids
}

fn collect_input_ids_inner(nodes: &[Node], ids: &mut Vec<NodeId>) {
    for node in nodes {
        match node {
            Node::Input(input) => ids.push(input.id().to_string()),
            Node::Component(component) => {
                if let Some(children) = component.children() {
                    collect_input_ids_inner(children, ids);
                }
            }
            _ => {}
        }
    }
}

fn collect_focus_targets(nodes: &[Node], prefix: &[usize]) -> Vec<FocusTarget> {
    let mut targets = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        let mut path = prefix.to_vec();
        path.push(idx);
        match node {
            Node::Input(input) => targets.push(FocusTarget {
                path,
                id: input.id().to_string(),
                kind: FocusKind::Input,
            }),
            Node::Component(component) => {
                if matches!(
                    component.focus_mode(),
                    crate::core::component::FocusMode::Group
                ) {
                    targets.push(FocusTarget {
                        path,
                        id: component.id().to_string(),
                        kind: FocusKind::Component,
                    });
                } else if let Some(children) = component.children() {
                    targets.extend(collect_focus_targets(children, &path));
                }
            }
            _ => {}
        }
    }
    targets
}

fn node_at_path_mut<'a>(nodes: &'a mut [Node], path: &[usize]) -> Option<&'a mut Node> {
    if path.is_empty() {
        return None;
    }
    let idx = *path.first()?;
    if idx >= nodes.len() {
        return None;
    }
    if path.len() == 1 {
        return nodes.get_mut(idx);
    }
    let node = nodes.get_mut(idx)?;
    let children = node.children_mut()?;
    node_at_path_mut(children, &path[1..])
}
