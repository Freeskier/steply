use crate::widgets::traits::{
    FocusMode, OverlayMode, OverlayPlacement, OverlayRenderMode, RenderContext,
};

// ---------------------------------------------------------------------------
// WidgetBase — shared identity for all interactive and output nodes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WidgetBase {
    id: String,
    label: String,
}

impl WidgetBase {
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

    /// The focus cursor prefix shown to the left of an input label.
    pub fn focus_marker(&self, focused: bool) -> &'static str {
        if focused { ">" } else { " " }
    }

    /// Formats `"> Label: "` or `"  Label: "` depending on focus state.
    pub fn input_prefix(&self, ctx: &RenderContext) -> String {
        let focused = ctx.focused_id.as_deref().is_some_and(|id| id == self.id());
        format!("{} {}: ", self.focus_marker(focused), self.label())
    }

    /// Returns whether this widget is currently focused.
    pub fn is_focused(&self, ctx: &RenderContext) -> bool {
        ctx.focused_id.as_deref().is_some_and(|id| id == self.id())
    }

    /// `"> Label: "` — always uses the focused marker (for cursor position math).
    pub fn input_prefix_focused(&self) -> String {
        format!("> {}: ", self.label())
    }
}

// ---------------------------------------------------------------------------
// OverlayBase — base for overlay components (modals, popovers, …)
// ---------------------------------------------------------------------------

/// Provides identity and placement configuration for overlay components.
///
/// Keep as a separate type from `WidgetBase` because overlays have additional
/// state (placement, focus mode, overlay mode) that plain inputs do not need.
/// This acts as a reusable template for custom overlay implementations.
#[derive(Debug, Clone)]
pub struct OverlayBase {
    id: String,
    label: String,
    placement: OverlayPlacement,
    focus_mode: FocusMode,
    overlay_mode: OverlayMode,
}

impl OverlayBase {
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

    pub fn focus_mode(&self) -> FocusMode {
        self.focus_mode
    }

    pub fn overlay_mode(&self) -> OverlayMode {
        self.overlay_mode
    }

    pub fn set_render_mode(&mut self, render_mode: OverlayRenderMode) {
        self.placement = self.placement.with_render_mode(render_mode);
    }

    pub fn set_focus_mode(&mut self, focus_mode: FocusMode) {
        self.focus_mode = focus_mode;
    }

    pub fn set_overlay_mode(&mut self, overlay_mode: OverlayMode) {
        self.overlay_mode = overlay_mode;
    }
}
