use crate::widgets::traits::{DrawOutput, Drawable, RenderContext, RenderNode};

pub struct Text {
    id: String,
    text: String,
}

impl Text {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl Drawable for Text {
    fn id(&self) -> &str {
        &self.id
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        DrawOutput::plain_lines(vec![self.text.clone()])
    }
}

impl RenderNode for Text {}
