#[derive(Debug, Clone, Default)]
pub struct FrameHitMap {
    regions: Vec<HitRegion>,
}

#[derive(Debug, Clone)]
pub struct HitRegion {
    pub node_id: String,
    pub row: u16,
    pub local_row: u16,
    pub col_start: u16,
    pub col_end_exclusive: u16,
    pub local_col_offset: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTarget<'a> {
    pub node_id: &'a str,
    pub local_row: u16,
    pub local_col: u16,
}

impl FrameHitMap {
    pub fn push_node_row(
        &mut self,
        node_id: impl Into<String>,
        row: u16,
        local_row: u16,
        col_start: u16,
        col_end_exclusive: u16,
        local_col_offset: u16,
    ) {
        if col_end_exclusive <= col_start {
            return;
        }
        self.regions.push(HitRegion {
            node_id: node_id.into(),
            row,
            local_row,
            col_start,
            col_end_exclusive,
            local_col_offset,
        });
    }

    pub fn push_node_rows(
        &mut self,
        node_id: impl Into<String>,
        row_start: u16,
        row_len: u16,
        col_start: u16,
        col_end_exclusive: u16,
        local_col_offset: u16,
    ) {
        if row_len == 0 {
            return;
        }
        let node_id = node_id.into();
        for (local_row, row) in (row_start..row_start.saturating_add(row_len)).enumerate() {
            self.push_node_row(
                node_id.clone(),
                row,
                local_row.min(u16::MAX as usize) as u16,
                col_start,
                col_end_exclusive,
                local_col_offset,
            );
        }
    }

    pub fn shift_rows(&mut self, delta: u16) {
        if delta == 0 {
            return;
        }
        for region in &mut self.regions {
            region.row = region.row.saturating_add(delta);
        }
    }

    pub fn shift_cols(&mut self, delta: u16) {
        if delta == 0 {
            return;
        }
        for region in &mut self.regions {
            region.col_start = region.col_start.saturating_add(delta);
            region.col_end_exclusive = region.col_end_exclusive.saturating_add(delta);
        }
    }

    pub fn insert_rows(&mut self, at: u16, count: u16) {
        if count == 0 {
            return;
        }
        for region in &mut self.regions {
            if region.row >= at {
                region.row = region.row.saturating_add(count);
            }
        }
    }

    pub fn extend(&mut self, mut other: FrameHitMap) {
        self.regions.append(&mut other.regions);
    }

    pub fn resolve(&self, row: u16, col: u16) -> Option<HitTarget<'_>> {
        let region = self.regions.iter().rev().find(|region| {
            row == region.row && col >= region.col_start && col < region.col_end_exclusive
        })?;

        let local_row = region.local_row;
        let local_col = col
            .saturating_sub(region.col_start)
            .saturating_sub(region.local_col_offset);
        Some(HitTarget {
            node_id: region.node_id.as_str(),
            local_row,
            local_col,
        })
    }
}
