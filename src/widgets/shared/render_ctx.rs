use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::widgets::traits::RenderContext;

pub fn child_context_for(
    parent_id: &str,
    parent_ctx: &RenderContext,
    focused_child_id: Option<String>,
) -> RenderContext {
    let mut completion_menus = HashMap::new();
    if let Some(child_id) = focused_child_id.as_deref()
        && let Some(menu) = parent_ctx.completion_menus.get(parent_id)
    {
        completion_menus.insert(child_id.to_string(), menu.clone());
    }

    RenderContext {
        focused_id: focused_child_id,
        terminal_size: parent_ctx.terminal_size,
        visible_errors: Arc::new(HashMap::new()),
        invalid_hidden: Arc::new(HashSet::new()),
        completion_menus: Arc::new(completion_menus),
    }
}
