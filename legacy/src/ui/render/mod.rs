mod decorator;
mod options;
mod pipeline;
mod render_context;
mod render_trait;
mod step_render;

pub use decorator::Decorator;
pub use options::RenderOptions;
pub use pipeline::RenderPipeline;
pub use render_context::RenderContext;
pub use render_trait::{Render, RenderCursor, RenderOutput};

#[derive(Debug, Clone)]
pub struct RenderLine {
    pub spans: Vec<crate::span::Span>,
}
