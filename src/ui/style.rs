#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Black,
    DarkGrey,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    color: Option<Color>,
    background: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(&self) -> Option<Color> {
        self.color
    }

    pub fn background(&self) -> Option<Color> {
        self.background
    }

    pub fn bold(&self) -> bool {
        self.bold
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn dim(&self) -> bool {
        self.dim
    }

    pub fn underline(&self) -> bool {
        self.underline
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_colors(mut self, color: Color, background: Color) -> Self {
        self.color = Some(color);
        self.background = Some(background);
        self
    }

    pub fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn with_italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn with_dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn with_underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn merge(mut self, other: &Style) -> Self {
        if other.color.is_some() {
            self.color = other.color;
        }
        if other.background.is_some() {
            self.background = other.background;
        }
        if other.bold {
            self.bold = true;
        }
        if other.dim {
            self.dim = true;
        }
        if other.italic {
            self.italic = true;
        }
        if other.underline {
            self.underline = true;
        }
        self
    }
}
