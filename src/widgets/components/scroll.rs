/// Shared scroll state used by list/tree components.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub max_visible: Option<usize>,
}

impl ScrollState {
    pub fn new(max_visible: Option<usize>) -> Self {
        Self {
            offset: 0,
            max_visible,
        }
    }

    /// Adjust offset so `active` is within the visible window.
    pub fn ensure_visible(&mut self, active: usize, total: usize) {
        let Some(max) = self.max_visible else {
            return;
        };
        if total <= max {
            self.offset = 0;
            return;
        }
        if active < self.offset {
            self.offset = active;
            return;
        }
        let last = self.offset.saturating_add(max).saturating_sub(1);
        if active > last {
            self.offset = active + 1 - max;
        }
    }

    /// Clamp `active` to `[0, total)`.
    pub fn clamp_active(active: &mut usize, total: usize) {
        if total == 0 {
            *active = 0;
        } else if *active >= total {
            *active = total - 1;
        }
    }

    /// Returns `(start, end)` indices into the visible slice.
    pub fn visible_range(&self, total: usize) -> (usize, usize) {
        match self.max_visible {
            Some(limit) => {
                let start = self.offset.min(total);
                let end = (start + limit).min(total);
                (start, end)
            }
            None => (0, total),
        }
    }

    /// Footer text like `[3-14 of 42] ↓` — `None` when no scroll needed.
    pub fn footer(&self, total: usize) -> Option<String> {
        let max = self.max_visible?;
        if total <= max {
            return None;
        }
        let (start, end) = self.visible_range(total);
        let can_up = start > 0;
        let can_down = end < total;
        let arrow = match (can_up, can_down) {
            (true, true) => " ↑↓",
            (true, false) => " ↑",
            (false, true) => " ↓",
            (false, false) => "",
        };
        Some(format!("[{}-{} of {}]{}", start + 1, end, total, arrow))
    }
}
