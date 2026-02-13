use crate::widgets::traits::{DrawOutput, Drawable, RenderContext, RenderNode};

pub struct TextOutput {
    id: String,
    text: String,
}

impl TextOutput {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl Drawable for TextOutput {
    fn id(&self) -> &str {
        &self.id
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput::plain_lines(vec![self.text.clone()])
    }
}

impl RenderNode for TextOutput {}
