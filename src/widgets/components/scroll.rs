



#[derive(Debug, Clone)]
pub struct CursorNav {
    active: usize,
    scroll: ScrollState,
}

impl CursorNav {
    pub fn new(max_visible: Option<usize>) -> Self {
        Self {
            active: 0,
            scroll: ScrollState::new(max_visible),
        }
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn set_max_visible(&mut self, n: usize) {
        self.scroll.max_visible = Some(n);
    }


    pub fn move_by(&mut self, delta: isize, total: usize) -> usize {
        if total == 0 {
            self.active = 0;
            return 0;
        }
        let len = total as isize;
        self.active = ((self.active as isize + delta + len) % len) as usize;
        self.scroll.ensure_visible(self.active, total);
        self.active
    }


    pub fn set_active(&mut self, idx: usize, total: usize) {
        self.active = idx;
        ScrollState::clamp_active(&mut self.active, total);
        self.scroll.ensure_visible(self.active, total);
    }


    pub fn clamp(&mut self, total: usize) {
        ScrollState::clamp_active(&mut self.active, total);
        self.scroll.ensure_visible(self.active, total);
    }

    pub fn visible_range(&self, total: usize) -> (usize, usize) {
        self.scroll.visible_range(total)
    }

    pub fn footer(&self, total: usize) -> Option<String> {
        self.scroll.footer(total)
    }

    pub fn ensure_visible(&mut self, total: usize) {
        self.scroll.ensure_visible(self.active, total);
    }
}


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


    pub fn clamp_active(active: &mut usize, total: usize) {
        if total == 0 {
            *active = 0;
        } else if *active >= total {
            *active = total - 1;
        }
    }


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
