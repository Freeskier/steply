#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub decorations_enabled: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            decorations_enabled: true,
        }
    }
}
