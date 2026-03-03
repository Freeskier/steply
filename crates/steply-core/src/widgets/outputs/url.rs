use crate::core::value::Value;
use crate::runtime::event::WidgetAction;
use crate::terminal::{PointerButton, PointerEvent, PointerKind};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, InteractionResult, OutputNode, RenderContext};

pub struct UrlOutput {
    id: String,
    name: Option<String>,
    url: String,
}

impl UrlOutput {
    pub fn new(id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            url: url.into(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    fn rendered_label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.url.clone())
    }

    fn osc8_link(url: &str, label: &str) -> String {
        // OSC 8 hyperlink: ESC ] 8 ;; URL ESC \ LABEL ESC ] 8 ;; ESC \
        format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
    }
}

impl Drawable for UrlOutput {
    fn id(&self) -> &str {
        &self.id
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let label = self.rendered_label();
        let linked = Self::osc8_link(self.url.as_str(), label.as_str());
        DrawOutput::with_lines(vec![vec![
            Span::styled(linked, Style::new().color(Color::Blue).bold()).no_wrap(),
            Span::styled("↗", Style::new().color(Color::DarkGrey)).no_wrap(),
        ]])
    }
}

impl OutputNode for UrlOutput {
    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        if matches!(event.kind, PointerKind::Down(PointerButton::Left)) {
            return InteractionResult::with_action(WidgetAction::OpenUrl {
                url: self.url.clone(),
            });
        }
        InteractionResult::ignored()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.url.clone()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar() {
            self.url = text;
        }
    }
}
