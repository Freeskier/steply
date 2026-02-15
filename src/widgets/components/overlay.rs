use crate::runtime::event::{OverlayLifecycle, SystemEvent};
use crate::terminal::{KeyCode, KeyEvent};
use crate::widgets::base::OverlayBase;
use crate::widgets::node::{Node, find_node_mut};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, OverlayMode, OverlayPlacement,
    OverlayRenderMode, RenderContext, TextAction,
};

pub struct Overlay {
    base: OverlayBase,
    nodes: Vec<Node>,
    group_focus_id: Option<String>,
}

impl Overlay {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        placement: OverlayPlacement,
        nodes: Vec<Node>,
    ) -> Self {
        Self {
            base: OverlayBase::new(id, label, placement),
            nodes,
            group_focus_id: None,
        }
    }

    pub fn placement(&self) -> OverlayPlacement {
        self.base.placement()
    }

    pub fn with_focus_mode(mut self, focus_mode: FocusMode) -> Self {
        self.base.set_focus_mode(focus_mode);
        self
    }

    pub fn with_overlay_mode(mut self, overlay_mode: OverlayMode) -> Self {
        self.base.set_overlay_mode(overlay_mode);
        self
    }

    pub fn with_render_mode(mut self, render_mode: OverlayRenderMode) -> Self {
        self.base.set_render_mode(render_mode);
        self
    }
}

impl crate::widgets::node::Component for Overlay {
    fn children(&self) -> &[Node] {
        &self.nodes
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut self.nodes
    }
}

impl Drawable for Overlay {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput::default()
    }
}

impl Interactive for Overlay {
    fn focus_mode(&self) -> FocusMode {
        self.base.focus_mode()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        Some(self.placement())
    }

    fn overlay_open(&mut self, _saved_focus_id: Option<String>) -> bool {
        self.group_focus_id = first_focusable_id(&self.nodes);
        true
    }

    fn overlay_close(&mut self) -> Option<String> {
        self.group_focus_id = None;
        None
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.base.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.base.focus_mode() == FocusMode::Group {
            let focusable = focusable_ids(&self.nodes);
            if focusable.is_empty() {
                return InteractionResult::ignored();
            }

            if self
                .group_focus_id
                .as_deref()
                .is_none_or(|id| !focusable.iter().any(|c| c == id))
            {
                self.group_focus_id = Some(focusable[0].clone());
            }

            let current_idx = self
                .group_focus_id
                .as_deref()
                .and_then(|id| focusable.iter().position(|c| c == id))
                .unwrap_or(0);

            match key.code {
                KeyCode::Tab => {
                    let next_idx = (current_idx + 1) % focusable.len();
                    self.group_focus_id = Some(focusable[next_idx].clone());
                    return InteractionResult::handled();
                }
                KeyCode::BackTab => {
                    let prev_idx = (current_idx + focusable.len() - 1) % focusable.len();
                    self.group_focus_id = Some(focusable[prev_idx].clone());
                    return InteractionResult::handled();
                }
                _ => {}
            }

            if let Some(focus_id) = self.group_focus_id.clone()
                && let Some(node) = find_node_mut(&mut self.nodes, &focus_id)
            {
                return node.on_key(key);
            }
        }
        InteractionResult::ignored()
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.base.focus_mode() != FocusMode::Group {
            return InteractionResult::ignored();
        }

        if let Some(focus_id) = self.group_focus_id.clone()
            && let Some(node) = find_node_mut(&mut self.nodes, &focus_id)
        {
            return node.on_text_action(action);
        }

        InteractionResult::ignored()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let targeted_lifecycle = matches!(
            event,
            SystemEvent::OverlayLifecycle { overlay_id, .. } if overlay_id.as_str() == self.base.id()
        );

        if let SystemEvent::OverlayLifecycle { phase, .. } = event {
            match phase {
                OverlayLifecycle::BeforeOpen | OverlayLifecycle::Opened => {
                    self.group_focus_id = first_focusable_id(&self.nodes);
                }
                OverlayLifecycle::BeforeClose
                | OverlayLifecycle::Closed
                | OverlayLifecycle::AfterClose => {
                    self.group_focus_id = None;
                }
            }
        }

        if let SystemEvent::RequestFocus { target } = event {
            let focusable = focusable_ids(&self.nodes);
            if focusable.iter().any(|c| c == target.as_str()) {
                self.group_focus_id = Some(target.to_string());
                return InteractionResult::handled();
            }
        }

        let mut merged = if targeted_lifecycle {
            InteractionResult::consumed()
        } else {
            InteractionResult::ignored()
        };

        for node in &mut self.nodes {
            merged.merge(node.on_system_event(event));
        }

        merged
    }
}

fn first_focusable_id(nodes: &[Node]) -> Option<String> {
    focusable_ids(nodes).into_iter().next()
}

fn focusable_ids(nodes: &[Node]) -> Vec<String> {
    let mut out = Vec::new();
    collect_focusable_ids(nodes, &mut out);
    out
}

fn collect_focusable_ids(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if node.is_focusable() {
            out.push(node.id().to_string());
            continue;
        }
        if let Some(children) = node.persistent_children() {
            collect_focusable_ids(children, out);
        }
    }
}
