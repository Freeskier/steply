use super::model::{SelectMode, SelectOption, option_text};
use std::collections::HashSet;

pub(super) fn clamp_active(active_index: &mut usize, options_len: usize) {
    if options_len == 0 {
        *active_index = 0;
    } else if *active_index >= options_len {
        *active_index = options_len - 1;
    }
}

pub(super) fn ensure_visible(
    scroll_offset: &mut usize,
    max_visible: Option<usize>,
    active_index: usize,
    options_len: usize,
) {
    let Some(max_visible) = max_visible else {
        return;
    };

    if options_len <= max_visible {
        *scroll_offset = 0;
        return;
    }

    if active_index < *scroll_offset {
        *scroll_offset = active_index;
        return;
    }

    let last_visible = scroll_offset.saturating_add(max_visible).saturating_sub(1);
    if active_index > last_visible {
        *scroll_offset = active_index + 1 - max_visible;
    }
}

pub(super) fn visible_range(
    scroll_offset: usize,
    max_visible: Option<usize>,
    options_len: usize,
) -> (usize, usize) {
    match max_visible {
        Some(limit) => {
            let start = scroll_offset.min(options_len);
            let end = (start + limit).min(options_len);
            (start, end)
        }
        None => (0, options_len),
    }
}

pub(super) fn marker_symbol(mode: SelectMode, selected: &[usize], index: usize) -> &'static str {
    let is_selected = selected.iter().any(|selected| *selected == index);
    match mode {
        SelectMode::Multi | SelectMode::Single => {
            if is_selected {
                "■"
            } else {
                "□"
            }
        }
        SelectMode::Radio => {
            if is_selected {
                "●"
            } else {
                "○"
            }
        }
        SelectMode::List => "",
    }
}

pub(super) fn apply_options_preserving_selection(
    options: &mut Vec<SelectOption>,
    selected: &mut Vec<usize>,
    active_index: &mut usize,
    scroll_offset: &mut usize,
    mode: SelectMode,
    new_options: Vec<SelectOption>,
) {
    let selected_values: HashSet<String> = selected
        .iter()
        .filter_map(|index| options.get(*index))
        .map(|option| option_text(option).to_string())
        .collect();

    *options = new_options;
    *selected = options
        .iter()
        .enumerate()
        .filter_map(|(idx, option)| {
            if selected_values.contains(option_text(option)) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if mode == SelectMode::Radio && selected.is_empty() && !options.is_empty() {
        selected.push(0);
    }

    if options.is_empty() {
        *active_index = 0;
        *scroll_offset = 0;
    } else {
        clamp_active(active_index, options.len());
    }
}
