use crate::runtime::event::WidgetEvent;
use crate::terminal::KeyEvent;
use crate::widgets::base::ModalBase;
use crate::widgets::node::{Node, find_node_mut};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, OverlayMode, OverlayPlacement,
    RenderContext,
};

pub struct Modal {
    base: ModalBase,
    nodes: Vec<Node>,
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
        }
    }

    pub fn open(&mut self, saved_focus_id: Option<String>) {
        self.base.open(saved_focus_id);
    }

    pub fn close(&mut self) -> Option<String> {
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

    fn on_key(&mut self, _key: KeyEvent) -> InteractionResult {
        if self.base.focus_mode() == FocusMode::Group
            && self.is_visible()
            && let Some(focus_id) = first_focusable_id(self.nodes.as_slice())
            && let Some(node) = find_node_mut(self.nodes.as_mut_slice(), &focus_id)
        {
            return node.on_key(_key);
        }
        InteractionResult::ignored()
    }

    fn on_event(&mut self, event: &WidgetEvent) -> InteractionResult {
        let targeted_lifecycle = matches!(
            event,
            WidgetEvent::OverlayLifecycle { overlay_id, .. } if overlay_id == self.base.id()
        );
        if !self.is_visible() && !targeted_lifecycle {
            return InteractionResult::ignored();
        }

        let mut merged = if targeted_lifecycle {
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        };

        if self.is_visible() {
            for node in self.nodes_mut() {
                let result = node.on_event(event);
                merged.handled |= result.handled;
                merged.events.extend(result.events);
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
}

fn first_focusable_id(nodes: &[Node]) -> Option<String> {
    for node in nodes {
        if node.is_focusable_leaf_or_group() {
            return Some(node.id().to_string());
        }
        if let Some(children) = node.children()
            && let Some(id) = first_focusable_id(children)
        {
            return Some(id);
        }
    }
    None
}
