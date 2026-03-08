use std::collections::HashSet;

use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::ui::text::text_display_width;
use crate::widgets::node::{Node, NodeWalkScope, walk_nodes};
use crate::widgets::traits::{HintContext, HintGroup, HintItem};

pub(super) fn render_hints_panel_lines(mut hints: Vec<HintItem>) -> Vec<SpanLine> {
    if hints.is_empty() {
        return Vec::new();
    }

    hints.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| a.label.cmp(&b.label))
    });

    let mut grouped = Vec::<(HintGroup, Vec<HintItem>)>::new();
    for group in [
        HintGroup::Navigation,
        HintGroup::Action,
        HintGroup::Completion,
        HintGroup::Edit,
        HintGroup::View,
    ] {
        let items = hints
            .iter()
            .filter(|hint| hint.group == group)
            .cloned()
            .collect::<Vec<_>>();
        if !items.is_empty() {
            grouped.push((group, items));
        }
    }

    grouped.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let column_widths = grouped
        .iter()
        .map(|(_, items)| {
            items
                .iter()
                .map(|item| {
                    let key_width = text_display_width(item.key.as_ref());
                    let label_width = text_display_width(item.label.as_ref());
                    if label_width == 0 {
                        key_width
                    } else {
                        key_width + 1 + label_width
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    let max_rows = grouped
        .iter()
        .map(|(_, items)| items.len())
        .max()
        .unwrap_or(0);
    const HINT_COLUMN_GAP: usize = 4;

    let mut lines = Vec::<SpanLine>::with_capacity(max_rows);
    for row in 0..max_rows {
        let mut line = Vec::<Span>::new();
        for (col_idx, (_, items)) in grouped.iter().enumerate() {
            let is_last_col = col_idx + 1 == grouped.len();
            if let Some(item) = items.get(row) {
                let key = item.key.to_string();
                let label = item.label.to_string();
                let key_style = Style::new().color(Color::DarkGrey).bold();
                let text_style = Style::new().color(Color::DarkGrey);
                line.push(Span::styled(key.clone(), key_style).no_wrap());
                if !label.is_empty() {
                    line.push(Span::styled(" ", text_style).no_wrap());
                    line.push(Span::styled(label.clone(), text_style).no_wrap());
                }

                if !is_last_col {
                    let rendered_width = if label.is_empty() {
                        text_display_width(key.as_str())
                    } else {
                        text_display_width(key.as_str()) + 1 + text_display_width(label.as_str())
                    };
                    let pad = column_widths[col_idx]
                        .saturating_sub(rendered_width)
                        .saturating_add(HINT_COLUMN_GAP);
                    if pad > 0 {
                        line.push(Span::new(" ".repeat(pad)).no_wrap());
                    }
                }
            } else if !is_last_col {
                let pad = column_widths[col_idx].saturating_add(HINT_COLUMN_GAP);
                if pad > 0 {
                    line.push(Span::new(" ".repeat(pad)).no_wrap());
                }
            }
        }
        lines.push(line);
    }
    lines
}

pub(super) fn collect_hints(nodes: &[Node], focused_id: Option<&str>) -> Vec<HintItem> {
    let mut out = Vec::<HintItem>::new();
    let mut seen = HashSet::<(String, String, HintGroup)>::new();
    walk_nodes(nodes, NodeWalkScope::TopLevel, &mut |node| {
        let focused = focused_id.is_some_and(|id| id == node.id());
        for hint in node.hints(HintContext {
            focused,
            expanded: true,
        }) {
            let key = hint.key.to_string();
            let label = hint.label.to_string();
            let dedup_key = (key.clone(), label.clone(), hint.group);
            if seen.insert(dedup_key) {
                out.push(HintItem {
                    key: key.into(),
                    label: label.into(),
                    priority: hint.priority,
                    group: hint.group,
                });
            }
        }
    });
    out
}
