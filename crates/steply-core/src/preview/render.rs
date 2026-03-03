use crate::preview::request::{RenderJsonRequest, RenderJsonScope};
use crate::state::app::AppState;
use crate::terminal::TerminalSize;
use crate::ui::frame_json::{draw_output_to_json, frame_to_json};
use crate::ui::render_view::RenderView;
use crate::ui::renderer::Renderer;
use crate::widgets::node::find_node;
use crate::widgets::traits::RenderContext;

pub fn render_json(
    state: &mut AppState,
    request: &RenderJsonRequest,
    renderer: &mut Renderer,
    default_size: TerminalSize,
) -> Result<serde_json::Value, String> {
    if let Some(step_id) = request.active_step_id.as_deref()
        && !state.set_current_step_by_id_for_preview(step_id)
    {
        return Err(format!("unknown active step id for render json: {step_id}"));
    }

    let size = request.terminal_size.unwrap_or(default_size);
    match &request.scope {
        RenderJsonScope::Current | RenderJsonScope::Flow => {
            let view = RenderView::from_state(state);
            let frame = renderer.render(&view, size);
            Ok(frame_to_json(&frame, size))
        }
        RenderJsonScope::Step { step_id } => {
            let Some(step_index) = state.step_index_by_id(step_id.as_str()) else {
                return Err(format!("unknown step id for render json: {step_id}"));
            };
            if !state.set_current_step_for_preview(step_index) {
                return Err(format!("cannot activate step for render json: {step_id}"));
            }
            let Some(view) = RenderView::from_state_step(state, step_index) else {
                return Err(format!("cannot build render view for step: {step_id}"));
            };
            let frame = renderer.render(&view, size);
            Ok(frame_to_json(&frame, size))
        }
        RenderJsonScope::Widget { step_id, widget_id } => {
            let Some(step_index) = state.step_index_by_id(step_id.as_str()) else {
                return Err(format!("unknown step id for widget render json: {step_id}"));
            };
            let Some(step) = state.steps().get(step_index) else {
                return Err(format!("missing step for widget render json: {step_id}"));
            };
            let Some(node) = find_node(step.nodes.as_slice(), widget_id.as_str()) else {
                return Err(format!(
                    "unknown widget id '{}' in step '{}'",
                    widget_id, step_id
                ));
            };
            let ctx = RenderContext::empty(size).with_focus(Some(widget_id.clone()));
            let output = node.draw(&ctx);
            Ok(draw_output_to_json(&output, size))
        }
    }
}
