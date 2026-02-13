use crate::widgets::traits::OverlayPlacement;
use crate::widgets::traits::{FocusMode, OverlayMode, OverlayRenderMode};

#[derive(Debug, Clone)]
pub struct InputBase {
    id: String,
    label: String,
}

impl InputBase {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn focus_marker(&self, focused: bool) -> &'static str {
        if focused { ">" } else { " " }
    }

    pub fn prefixed_label(&self, focused: bool) -> String {
        format!("{} {}", self.focus_marker(focused), self.label)
    }
}

#[derive(Debug, Clone)]
pub struct ComponentBase {
    id: String,
    label: String,
}

impl ComponentBase {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn focus_marker(&self, focused: bool) -> &'static str {
        if focused { ">" } else { " " }
    }
}

#[derive(Debug, Clone)]
pub struct ModalBase {
    id: String,
    label: String,
    placement: OverlayPlacement,
    focus_mode: FocusMode,
    overlay_mode: OverlayMode,
}

impl ModalBase {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        placement: OverlayPlacement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placement,
            focus_mode: FocusMode::Container,
            overlay_mode: OverlayMode::Exclusive,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn placement(&self) -> OverlayPlacement {
        self.placement
    }

    pub fn set_render_mode(&mut self, render_mode: OverlayRenderMode) {
        self.placement = self.placement.with_render_mode(render_mode);
    }

    pub fn focus_mode(&self) -> FocusMode {
        self.focus_mode
    }

    pub fn set_focus_mode(&mut self, focus_mode: FocusMode) {
        self.focus_mode = focus_mode;
    }

    pub fn overlay_mode(&self) -> OverlayMode {
        self.overlay_mode
    }

    pub fn set_overlay_mode(&mut self, overlay_mode: OverlayMode) {
        self.overlay_mode = overlay_mode;
    }

    pub fn open(&mut self) {}

    pub fn close(&mut self) {}
}
