use crate::core::flow::StepStatus;
use crate::ui::frame::Line;
use crate::ui::render::options::RenderOptions;
use crate::ui::span::{Span, Wrap};
use crate::ui::style::Style;
use crate::ui::theme::Theme;

pub struct Decorator<'a> {
    theme: &'a Theme,
}

impl<'a> Decorator<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    pub fn decorate(&self, lines: Vec<Line>, options: &RenderOptions) -> Vec<Line> {
        let (glyph, style) = self.status_glyph(options.status);

        let mut decorated: Vec<Line> = lines
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let prefix = if idx == 0 {
                    format!("{}  ", glyph)
                } else {
                    "│  ".to_string()
                };
                self.prepend_to_line(line, &prefix, &style)
            })
            .collect();

        if !options.connect_to_next {
            decorated.push(self.corner_line(&style));
        }

        decorated
    }

    fn status_glyph(&self, status: StepStatus) -> (&'static str, Style) {
        match status {
            StepStatus::Active => ("◇", self.theme.decor_active.clone()),
            StepStatus::Done => ("◈", self.theme.decor_done.clone()),
            StepStatus::Cancelled => ("◆", self.theme.decor_cancelled.clone()),
            StepStatus::Pending => ("◇", self.theme.decor_done.clone()),
        }
    }

    fn prepend_to_line(&self, mut line: Line, prefix: &str, style: &Style) -> Line {
        let mut new_line = Line::new();
        new_line.push(
            Span::new(prefix)
                .with_style(style.clone())
                .with_wrap(Wrap::No),
        );
        for span in line.take_spans() {
            new_line.push(span);
        }
        new_line
    }

    fn corner_line(&self, style: &Style) -> Line {
        let mut line = Line::new();
        line.push(
            Span::new("└  ")
                .with_style(style.clone())
                .with_wrap(Wrap::No),
        );
        line
    }
}
