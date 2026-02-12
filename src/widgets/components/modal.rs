use crate::runtime::event::OverlayLifecycle;
use crate::runtime::event::WidgetEvent;
use crate::terminal::{KeyCode, KeyEvent};
use crate::widgets::base::ModalBase;
use crate::widgets::node::{Node, find_node_mut};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, OverlayMode, OverlayPlacement,
    OverlayRenderMode, RenderContext, TextAction,
};

pub struct Modal {
    base: ModalBase,
    nodes: Vec<Node>,
    group_focus_id: Option<String>,
}

impl Modal {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        placement: OverlayPlacement,
        nodes: Vec<Node>,
    ) -> Self {
        Self {
            base: ModalBase::new(id, label, placement),
            nodes,
            group_focus_id: None,
        }
    }

    pub fn open(&mut self, saved_focus_id: Option<String>) {
        self.base.open(saved_focus_id);
        self.group_focus_id = first_focusable_id(self.nodes.as_slice());
    }

    pub fn close(&mut self) -> Option<String> {
        self.group_focus_id = None;
        self.base.close()
    }

    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }

    pub fn id(&self) -> &str {
        self.base.id()
    }

    pub fn label(&self) -> &str {
        self.base.label()
    }

    pub fn placement(&self) -> OverlayPlacement {
        self.base.placement()
    }

    pub fn nodes(&self) -> &[Node] {
        self.nodes.as_slice()
    }

    pub fn nodes_mut(&mut self) -> &mut [Node] {
        self.nodes.as_mut_slice()
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

impl Drawable for Modal {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput::default()
    }
}

impl Interactive for Modal {
    fn focus_mode(&self) -> FocusMode {
        self.base.focus_mode()
    }

    fn overlay_placement(&self) -> Option<OverlayPlacement> {
        Some(self.placement())
    }

    fn overlay_is_visible(&self) -> bool {
        self.is_visible()
    }

    fn overlay_open(&mut self, saved_focus_id: Option<String>) -> bool {
        self.open(saved_focus_id);
        true
    }

    fn overlay_close(&mut self) -> Option<String> {
        self.close()
    }

    fn overlay_mode(&self) -> OverlayMode {
        self.base.overlay_mode()
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if self.base.focus_mode() == FocusMode::Group && self.is_visible() {
            let focusable = focusable_ids(self.nodes.as_slice());
            if focusable.is_empty() {
                return InteractionResult::ignored();
            }

            if self
                .group_focus_id
                .as_deref()
                .is_none_or(|id| !focusable.iter().any(|candidate| candidate == id))
            {
                self.group_focus_id = Some(focusable[0].clone());
            }

            let current_idx = self
                .group_focus_id
                .as_deref()
                .and_then(|id| focusable.iter().position(|candidate| candidate == id))
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

            if let Some(focus_id) = self.group_focus_id.as_deref()
                && let Some(node) = find_node_mut(self.nodes.as_mut_slice(), focus_id)
            {
                return node.on_key(key);
            }
        }
        InteractionResult::ignored()
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if self.base.focus_mode() != FocusMode::Group || !self.is_visible() {
            return InteractionResult::ignored();
        }

        if let Some(focus_id) = self.group_focus_id.as_deref()
            && let Some(node) = find_node_mut(self.nodes.as_mut_slice(), focus_id)
        {
            return node.on_text_action(action);
        }

        InteractionResult::ignored()
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        let targeted_lifecycle = matches!(
            event,
            WidgetEvent::OverlayLifecycle { overlay_id, .. } if overlay_id.as_str() == self.base.id()
        );
        if !self.is_visible() && !targeted_lifecycle {
            return InteractionResult::ignored();
        }

        if let WidgetEvent::OverlayLifecycle { phase, .. } = event {
            match phase {
                OverlayLifecycle::BeforeOpen | OverlayLifecycle::Opened => {
                    self.group_focus_id = first_focusable_id(self.nodes.as_slice());
                }
                OverlayLifecycle::BeforeClose
                | OverlayLifecycle::Closed
                | OverlayLifecycle::AfterClose => {
                    self.group_focus_id = None;
                }
            }
        }

        if let WidgetEvent::RequestFocus { target } = event {
            let focusable = focusable_ids(self.nodes.as_slice());
            if focusable
                .iter()
                .any(|candidate| candidate == target.as_str())
            {
                self.group_focus_id = Some(target.as_str().to_string());
                return InteractionResult::handled();
            }
        }

        let mut merged = if targeted_lifecycle {
            InteractionResult::consumed()
        } else {
            InteractionResult::ignored()
        };

        if self.is_visible() || targeted_lifecycle {
            for node in self.nodes_mut() {
                merged.merge(node.on_event(event));
            }
        }

        merged
    }

    fn children(&self) -> Option<&[Node]> {
        if self.is_visible() {
            Some(self.nodes())
        } else {
            None
        }
    }

    fn children_mut(&mut self) -> Option<&mut [Node]> {
        if self.is_visible() {
            Some(self.nodes_mut())
        } else {
            None
        }
    }

    fn state_children(&self) -> Option<&[Node]> {
        Some(self.nodes())
    }

    fn state_children_mut(&mut self) -> Option<&mut [Node]> {
        Some(self.nodes_mut())
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
        if node.is_focusable_leaf_or_group() {
            out.push(node.id().to_string());
            continue;
        }
        if let Some(children) = node.children() {
            collect_focusable_ids(children, out);
        }
    }
}
