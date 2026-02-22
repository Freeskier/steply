#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Reset,
    Black,
    DarkGrey,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strike {
    #[default]
    Inherit,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub color: Option<Color>,
    pub background: Option<Color>,
    pub bold: bool,




    pub strike: Strike,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strike = Strike::On;
        self
    }

    pub fn no_strikethrough(mut self) -> Self {
        self.strike = Strike::Off;
        self
    }

    pub fn strike(mut self, strike: Strike) -> Self {
        self.strike = strike;
        self
    }

    pub fn merge(self, extra: Style) -> Self {
        Self {
            color: extra.color.or(self.color),
            background: extra.background.or(self.background),
            bold: self.bold || extra.bold,
            strike: match extra.strike {
                Strike::Inherit => self.strike,
                s => s,
            },
        }
    }

    pub fn merge_no_inherit(self, extra: Style) -> Self {
        Self {
            color: extra.color.or(self.color),
            background: extra.background.or(self.background),
            bold: extra.bold,
            strike: extra.strike,
        }
    }
}
