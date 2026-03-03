use crate::terminal::TerminalSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderJsonScope {
    Current,
    Flow,
    Step { step_id: String },
    Widget { step_id: String, widget_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderJsonRequest {
    pub scope: RenderJsonScope,
    pub active_step_id: Option<String>,
    pub terminal_size: Option<TerminalSize>,
}

impl Default for RenderJsonRequest {
    fn default() -> Self {
        Self {
            scope: RenderJsonScope::Current,
            active_step_id: None,
            terminal_size: None,
        }
    }
}
