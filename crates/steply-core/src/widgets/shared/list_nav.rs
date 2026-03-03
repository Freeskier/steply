use crate::widgets::shared::list_policy;

pub fn apply_cycle_index(current: &mut usize, len: usize, reverse: bool) -> bool {
    let next = if reverse {
        list_policy::cycle_prev(*current, len)
    } else {
        list_policy::cycle_next(*current, len)
    };
    let Some(next) = next else {
        return false;
    };
    if *current == next {
        return false;
    }
    *current = next;
    true
}

pub fn apply_move_index(current: &mut usize, len: usize, delta: isize) -> bool {
    let Some(next) = list_policy::move_by(*current, delta, len) else {
        return false;
    };
    if *current == next {
        return false;
    }
    *current = next;
    true
}
