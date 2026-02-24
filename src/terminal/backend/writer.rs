use super::*;

impl Terminal {
    pub(super) fn write_span_line(&mut self, line: &SpanLine, width: u16) -> io::Result<()> {
        self.write_span_line_with_margin(line, width, true)
    }

    fn write_span_line_with_margin(
        &mut self,
        line: &SpanLine,
        width: u16,
        keep_one_cell_margin: bool,
    ) -> io::Result<()> {
        let render_width = if keep_one_cell_margin && width > 1 {
            width - 1
        } else {
            width
        };
        let mut used = 0usize;
        for span in line {
            if used >= render_width as usize {
                break;
            }
            let available_cols = (render_width as usize).saturating_sub(used);
            let clipped = clip_to_display_width_without_linebreaks(&span.text, available_cols);
            if clipped.is_empty() {
                continue;
            }
            if let Some(color) = span.style.color {
                queue!(self.stdout, SetForegroundColor(map_color(color)))?;
            }
            if let Some(background) = span.style.background {
                queue!(self.stdout, SetBackgroundColor(map_color(background)))?;
            }
            if span.style.bold {
                queue!(self.stdout, SetAttribute(Attribute::Bold))?;
            }
            if matches!(span.style.strike, Strike::On) {
                queue!(self.stdout, SetAttribute(Attribute::CrossedOut))?;
            }
            queue!(self.stdout, Print(clipped.as_str()), ResetColor)?;
            if span.style.bold {
                queue!(self.stdout, SetAttribute(Attribute::NormalIntensity))?;
            }
            if matches!(span.style.strike, Strike::On) {
                queue!(self.stdout, SetAttribute(Attribute::NotCrossedOut))?;
            }
            used = used.saturating_add(text_display_width(clipped.as_str()));
        }
        Ok(())
    }

    pub(super) fn print_frame_to_stdout(
        &mut self,
        lines: &[SpanLine],
        width: u16,
    ) -> io::Result<()> {
        for line in lines {
            self.write_span_line_with_margin(line, width, false)?;
            self.stdout.write_all(b"\r\n")?;
        }
        Ok(())
    }
}

fn map_color(color: Color) -> CrosstermColor {
    match color {
        Color::Reset => CrosstermColor::Reset,
        Color::Black => CrosstermColor::Black,
        Color::DarkGrey => CrosstermColor::DarkGrey,
        Color::Red => CrosstermColor::Red,
        Color::Green => CrosstermColor::Green,
        Color::Yellow => CrosstermColor::DarkYellow,
        Color::Blue => CrosstermColor::DarkBlue,
        Color::Magenta => CrosstermColor::DarkMagenta,
        Color::Cyan => CrosstermColor::DarkCyan,
        Color::White => CrosstermColor::White,
        Color::Rgb(r, g, b) => CrosstermColor::Rgb { r, g, b },
    }
}
