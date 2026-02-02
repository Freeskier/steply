use crate::style::{Color, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub prompt: Style,
    pub hint: Style,
    pub error: Style,
    pub placeholder: Style,
    pub focused: Style,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            prompt: Style::new().with_bold(),
            hint: Style::new().with_color(Color::DarkGrey),
            error: Style::new().with_color(Color::Red).with_bold(),
            placeholder: Style::new().with_color(Color::DarkGrey),
            focused: Style::new().with_bold(),
        }
    }
}
