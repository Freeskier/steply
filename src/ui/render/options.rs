use crate::core::flow::StepStatus;

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub status: StepStatus,
    pub connect_to_next: bool,
}

impl RenderOptions {
    pub fn active() -> Self {
        Self {
            status: StepStatus::Active,
            connect_to_next: false,
        }
    }

    pub fn done() -> Self {
        Self {
            status: StepStatus::Done,
            connect_to_next: true,
        }
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self::active()
    }
}
