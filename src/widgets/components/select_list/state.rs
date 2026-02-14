use super::model::{SelectMode, SelectOption, option_text};
use std::collections::HashSet;

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
    } else if *active_index >= options.len() {
        *active_index = options.len() - 1;
    }
}
