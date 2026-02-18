use super::SelectMode;
use super::model::{SelectItem, SelectItemView};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::Style;
use crate::widgets::inputs::text_edit;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct SelectItemRenderState {
    pub focused: bool,
    pub active: bool,
    pub selected: bool,
    pub mode: SelectMode,
    pub base_style: Style,
    pub highlight_style: Style,
}

pub type OptionRenderer =
    Arc<dyn Fn(&SelectItem, SelectItemRenderState) -> Vec<SpanLine> + Send + Sync>;

pub fn default_option_renderer() -> OptionRenderer {
    Arc::new(default_render_option_lines)
}

fn default_render_option_lines(item: &SelectItem, state: SelectItemRenderState) -> Vec<SpanLine> {
    let base_style = state.base_style;
    let highlight_style = state.highlight_style;

    match &item.view {
        SelectItemView::Plain { text, highlights } => vec![render_text_spans(
            text.as_str(),
            highlights,
            base_style,
            highlight_style,
        )],
        SelectItemView::Detailed {
            title,
            description,
            title_highlights,
            description_highlights,
            title_style,
            description_style,
        } => {
            let title_base = base_style.merge(*title_style);
            let description_base = base_style.merge_no_inherit(*description_style);
            let mut lines = vec![render_text_spans(
                title.as_str(),
                title_highlights,
                title_base,
                highlight_style,
            )];
            if !description.is_empty() {
                lines.push(render_text_spans(
                    description.as_str(),
                    description_highlights,
                    description_base,
                    highlight_style,
                ));
            }
            lines
        }
        SelectItemView::Styled {
            text,
            highlights,
            style,
        } => vec![render_text_spans(
            text.as_str(),
            highlights,
            base_style.merge(*style),
            highlight_style,
        )],
        SelectItemView::Split {
            text,
            name_start,
            highlights,
            prefix_style,
            name_style,
        } => {
            let (prefix, name) = split_text_at_char(text.as_str(), *name_start);
            let mut spans = Vec::<Span>::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix, *prefix_style).no_wrap());
            }
            spans.extend(render_text_spans(
                name.as_str(),
                highlights,
                base_style.merge(*name_style),
                highlight_style,
            ));
            vec![spans]
        }
        SelectItemView::Suffix {
            text,
            highlights,
            suffix_start,
            style,
            suffix_style,
        } => {
            let (name, suffix) = split_text_at_char(text.as_str(), *suffix_start);
            let mut spans = render_text_spans(
                name.as_str(),
                highlights,
                base_style.merge(*style),
                highlight_style,
            );
            if !suffix.is_empty() {
                spans.push(Span::styled(suffix, base_style.merge(*suffix_style)));
            }
            vec![spans]
        }
        SelectItemView::SplitSuffix {
            text,
            name_start,
            suffix_start,
            highlights,
            prefix_style,
            name_style,
            suffix_style,
        } => {
            let (prefix, rest) = split_text_at_char(text.as_str(), *name_start);
            let name_len = suffix_start.saturating_sub(*name_start);
            let (name, suffix) = split_text_at_char(rest.as_str(), name_len);

            let mut spans = Vec::<Span>::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix, *prefix_style).no_wrap());
            }

            spans.extend(render_text_spans(
                name.as_str(),
                highlights,
                base_style.merge(*name_style),
                highlight_style,
            ));
            if !suffix.is_empty() {
                spans.push(Span::styled(suffix, base_style.merge(*suffix_style)));
            }
            vec![spans]
        }
    }
}

fn split_text_at_char(text: &str, char_index: usize) -> (String, String) {
    let byte_index = text_edit::byte_index_at_char(text, char_index);
    (
        text[..byte_index].to_string(),
        text[byte_index..].to_string(),
    )
}
