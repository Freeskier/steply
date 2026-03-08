use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::state::validation::ValidationState;
use crate::terminal::TerminalSize;
use crate::ui::render_view::CompletionSnapshot;
use crate::ui::span::SpanLine;
use crate::ui::style::Color;
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::traits::{CompletionMenu, RenderContext};

use super::StepVisualStatus;

pub(super) fn render_context_for_nodes(
    validation: &ValidationState,
    completion: Option<&CompletionSnapshot>,
    terminal_size: TerminalSize,
    status: StepVisualStatus,
    nodes: &[Node],
    focused_id: Option<&str>,
) -> RenderContext {
    if status != StepVisualStatus::Active {
        return RenderContext::empty(terminal_size);
    }

    let mut visible_errors = HashMap::<String, String>::new();
    let mut invalid_hidden = HashSet::<String>::new();
    let mut completion_menus = HashMap::<String, CompletionMenu>::new();
    walk_nodes(nodes, NodeWalkScope::TopLevel, &mut |node| {
        if let Some(error) = validation.visible_error(node.id()) {
            visible_errors.insert(node.id().to_string(), error.to_string());
        } else if validation.is_hidden_invalid(node.id()) {
            invalid_hidden.insert(node.id().to_string());
        }
    });

    if let Some(snap) = completion {
        completion_menus.insert(
            snap.owner.clone(),
            CompletionMenu {
                matches: snap.matches.clone(),
                selected: snap.selected,
                start: snap.start,
            },
        );
    }

    RenderContext {
        focused_id: focused_id.map(ToOwned::to_owned),
        terminal_size,
        visible_errors: Arc::new(visible_errors),
        invalid_hidden: Arc::new(invalid_hidden),
        completion_menus: Arc::new(completion_menus),
    }
}

pub(super) fn tint_block(lines: &mut [SpanLine], color: Color) {
    for line in lines {
        for span in line {
            span.style.color = Some(color);
        }
    }
}
