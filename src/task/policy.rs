#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RerunPolicy {
    Never,
    #[default]
    Always,
    IfChanged,
    Cooldown {
        ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConcurrencyPolicy {
    DropNew,
    #[default]
    Restart,
    Queue,
    Parallel,
}
