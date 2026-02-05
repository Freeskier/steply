mod decorator;
mod options;
mod pipeline;
mod step_renderer;

pub use decorator::Decorator;
pub use options::RenderOptions;
pub use pipeline::RenderPipeline;
pub use step_renderer::{RenderContext, StepRenderer};

pub struct RenderLine {
    pub spans: Vec<crate::span::Span>,
    pub cursor_offset: Option<usize>,
}
