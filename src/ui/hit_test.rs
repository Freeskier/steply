use crate::terminal::PointerSemantic;

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
    pub local_semantic: PointerSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitLocal {
    pub row: u16,
    pub col_offset: u16,
    pub semantic: PointerSemantic,
}

impl HitLocal {
    pub fn row(row: u16) -> Self {
        Self {
            row,
            col_offset: 0,
            semantic: PointerSemantic::None,
        }
    }

    pub fn with_col_offset(mut self, col_offset: u16) -> Self {
        self.col_offset = col_offset;
        self
    }

    pub fn with_semantic(mut self, semantic: PointerSemantic) -> Self {
        self.semantic = semantic;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTarget<'a> {
    pub node_id: &'a str,
    pub local_row: u16,
    pub local_col: u16,
    pub local_semantic: PointerSemantic,
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
        self.push_node_row_with_semantic(
            node_id,
            row,
            col_start,
            col_end_exclusive,
            HitLocal::row(local_row).with_col_offset(local_col_offset),
        );
    }

    pub fn push_node_row_with_semantic(
        &mut self,
        node_id: impl Into<String>,
        row: u16,
        col_start: u16,
        col_end_exclusive: u16,
        local: HitLocal,
    ) {
        if col_end_exclusive <= col_start {
            return;
        }
        self.regions.push(HitRegion {
            node_id: node_id.into(),
            row,
            local_row: local.row,
            col_start,
            col_end_exclusive,
            local_col_offset: local.col_offset,
            local_semantic: local.semantic,
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
            local_semantic: region.local_semantic,
        })
    }
}
