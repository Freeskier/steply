#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Digit,
    Alpha,
    Alnum,
    NumericRange { min: i64, max: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentRole {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentSpec {
    pub kind: SegmentKind,
    pub min_len: usize,
    pub max_len: Option<usize>,
    pub role: Option<SegmentRole>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskToken {
    Literal(char),
    Segment(SegmentSpec),
}
