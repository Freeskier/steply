use crate::ui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
    Radio,
    List,
}

#[derive(Debug, Clone)]
pub enum SelectOption {
    Plain(String),
    Highlighted {
        text: String,
        highlights: Vec<(usize, usize)>,
    },
    Styled {
        text: String,
        highlights: Vec<(usize, usize)>,
        style: Style,
    },
    Split {
        text: String,
        name_start: usize,
        highlights: Vec<(usize, usize)>,
        prefix_style: Style,
        name_style: Style,
    },
    Suffix {
        text: String,
        highlights: Vec<(usize, usize)>,
        suffix_start: usize,
        style: Style,
        suffix_style: Style,
    },
    SplitSuffix {
        text: String,
        name_start: usize,
        suffix_start: usize,
        highlights: Vec<(usize, usize)>,
        prefix_style: Style,
        name_style: Style,
        suffix_style: Style,
    },
}

impl SelectOption {
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain(text.into())
    }
}

pub(super) fn option_text(option: &SelectOption) -> &str {
    match option {
        SelectOption::Plain(text) => text.as_str(),
        SelectOption::Highlighted { text, .. } => text.as_str(),
        SelectOption::Styled { text, .. } => text.as_str(),
        SelectOption::Split { text, .. } => text.as_str(),
        SelectOption::Suffix { text, .. } => text.as_str(),
        SelectOption::SplitSuffix { text, .. } => text.as_str(),
    }
}
