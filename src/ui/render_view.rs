use crate::state::app::AppState;
use crate::state::step::{Step, StepStatus};
use crate::state::validation::ValidationState;
use crate::widgets::node::Node;
use crate::widgets::traits::OverlayPlacement;

pub struct RenderView<'a> {
    pub steps: &'a [Step],
    pub current_step_index: usize,
    pub step_statuses: Vec<StepStatus>,
    pub has_blocking_overlay: bool,
    pub focused_id: Option<&'a str>,
    pub step_errors: &'a [String],
    pub step_warnings: &'a [String],
    pub validation: &'a ValidationState,
    pub completion: Option<CompletionSnapshot>,
    pub overlays: Vec<OverlayView<'a>>,
    pub back_confirm: Option<&'a str>,
    pub hints_visible: bool,
}

pub struct CompletionSnapshot {
    pub owner: String,
    pub matches: Vec<String>,
    pub selected: usize,
    pub start: usize,
}

pub struct OverlayView<'a> {
    pub placement: OverlayPlacement,
    pub nodes: &'a [Node],
    pub is_topmost: bool,
}

impl<'a> RenderView<'a> {
    pub fn from_state(state: &'a AppState) -> Self {
        let steps = state.steps();
        let current_step_index = state.current_step_index();

        let step_statuses: Vec<StepStatus> =
            (0..steps.len()).map(|i| state.step_status_at(i)).collect();

        let overlay_ids = state.overlay_stack_ids();
        let overlay_count = overlay_ids.len();
        let mut overlays = Vec::with_capacity(overlay_count);
        for (idx, overlay_id) in overlay_ids.iter().enumerate() {
            let Some(overlay) = state.overlay_by_id(overlay_id) else {
                continue;
            };
            let Some(placement) = overlay.overlay_placement() else {
                continue;
            };
            let nodes = overlay.persistent_children().unwrap_or(&[]);
            overlays.push(OverlayView {
                placement,
                nodes,
                is_topmost: idx + 1 == overlay_count,
            });
        }

        let completion = state
            .completion_snapshot()
            .map(|(owner, matches, selected, start)| CompletionSnapshot {
                owner,
                matches,
                selected,
                start,
            });

        Self {
            steps,
            current_step_index,
            step_statuses,
            has_blocking_overlay: state.has_blocking_overlay(),
            focused_id: state.focused_id(),
            step_errors: state.current_step_errors(),
            step_warnings: state.current_step_warnings(),
            validation: state.validation_state(),
            completion,
            overlays,
            back_confirm: state.back_confirm(),
            hints_visible: state.hints_visible(),
        }
    }
}
