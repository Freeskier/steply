use crate::widgets::shared::filter::FilterController;

pub fn clamp_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        current.min(len.saturating_sub(1))
    }
}

pub fn cycle_next(current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((current + 1) % len)
}

pub fn cycle_prev(current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((current + len - 1) % len)
}

pub fn move_by(current: usize, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let next = current.checked_add_signed(delta)?;
    if next >= len {
        return None;
    }
    Some(next)
}

pub fn toggle_filter_visibility(
    filter: &mut FilterController,
    clear_on_hide: bool,
) -> bool {
    filter.toggle_visibility(clear_on_hide)
}

pub fn collect_matching_indices(
    total: usize,
    mut predicate: impl FnMut(usize) -> bool,
) -> Vec<usize> {
    let mut out = Vec::new();
    for index in 0..total {
        if predicate(index) {
            out.push(index);
        }
    }
    out
}

pub fn prefer_visible_index(
    visible: &[usize],
    preferred: Option<usize>,
    current: usize,
) -> Option<usize> {
    if visible.is_empty() {
        return None;
    }
    if let Some(preferred) = preferred && visible.contains(&preferred) {
        return Some(preferred);
    }
    if visible.contains(&current) {
        return Some(current);
    }
    visible.first().copied()
}
