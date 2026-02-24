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
