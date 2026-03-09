use crate::widgets::traits::RenderContext;

pub fn child_context_for(
    parent_id: &str,
    parent_ctx: &RenderContext,
    focused_child_id: Option<String>,
) -> RenderContext {
    parent_ctx.for_child(parent_id, focused_child_id)
}
