#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerunPolicy {
    Never,
    Always,
    IfChanged,
    Cooldown { ms: u64 },
}

impl Default for RerunPolicy {
    fn default() -> Self {
        Self::Always
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencyPolicy {
    DropNew,
    Restart,
    Queue,
    Parallel,
}

impl Default for ConcurrencyPolicy {
    fn default() -> Self {
        Self::Restart
    }
}
