use crate::style::{Color, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub prompt: Style,
    pub hint: Style,
    pub error: Style,
    pub placeholder: Style,
    pub focused: Style,
    pub decor_active: Style,
    pub decor_accent: Style,
    pub decor_done: Style,
    pub decor_cancelled: Style,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            prompt: Style::new().with_bold(),
            hint: Style::new().with_color(Color::DarkGrey),
            error: Style::new().with_color(Color::Red).with_bold(),
            placeholder: Style::new().with_color(Color::DarkGrey),
            focused: Style::new().with_bold(),
            decor_active: Style::new().with_color(Color::Green),
            decor_accent: Style::new().with_color(Color::Yellow),
            decor_done: Style::new().with_color(Color::DarkGrey),
            decor_cancelled: Style::new().with_color(Color::Red),
        }
    }
}
