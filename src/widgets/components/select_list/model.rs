use crate::core::value::Value;
use crate::ui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
    Radio,
    List,
}

#[derive(Debug, Clone)]
pub struct SelectItem {
    pub value: Value,
    pub search_text: String,
    pub view: SelectItemView,
}

#[derive(Debug, Clone)]
pub enum SelectItemView {
    Plain {
        text: String,
        highlights: Vec<(usize, usize)>,
    },
    Detailed {
        title: String,
        description: String,
        title_highlights: Vec<(usize, usize)>,
        description_highlights: Vec<(usize, usize)>,
        title_style: Style,
        description_style: Style,
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

impl SelectItem {
    pub fn new(value: Value, view: SelectItemView) -> Self {
        let search_text = search_text_from_view(&view);
        Self {
            value,
            search_text,
            view,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            value: Value::Text(text.clone()),
            search_text: text.clone(),
            view: SelectItemView::Plain {
                text,
                highlights: Vec::new(),
            },
        }
    }

    pub fn detailed(
        value: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let value = value.into();
        let title = title.into();
        let description = description.into();
        Self {
            value: Value::Text(value.clone()),
            search_text: format!("{value} {title} {description}"),
            view: SelectItemView::Detailed {
                title,
                description,
                title_highlights: Vec::new(),
                description_highlights: Vec::new(),
                title_style: Style::new().bold(),
                description_style: Style::new().color(Color::DarkGrey),
            },
        }
    }

    pub fn with_value(mut self, value: Value) -> Self {
        self.value = value;
        self
    }

    pub fn with_search_text(mut self, text: impl Into<String>) -> Self {
        self.search_text = text.into();
        self
    }
}

pub(super) fn item_search_text(item: &SelectItem) -> &str {
    item.search_text.as_str()
}

fn search_text_from_view(view: &SelectItemView) -> String {
    match view {
        SelectItemView::Plain { text, .. }
        | SelectItemView::Styled { text, .. }
        | SelectItemView::Split { text, .. }
        | SelectItemView::Suffix { text, .. }
        | SelectItemView::SplitSuffix { text, .. } => text.clone(),
        SelectItemView::Detailed {
            title, description, ..
        } => format!("{title} {description}"),
    }
}
