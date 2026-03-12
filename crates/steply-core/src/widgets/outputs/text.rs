use crate::core::value::Value;
use crate::widgets::traits::{DrawOutput, Drawable, OutputNode, RenderContext};

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

impl OutputNode for TextOutput {
    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.text.clone()))
    }

    fn set_value(&mut self, value: Value) {
        self.text = value.to_text_scalar().unwrap_or_else(|| value.to_json());
    }
}
