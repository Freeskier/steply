use super::*;

impl Terminal {
    pub fn enter(&mut self) -> io::Result<()> {
        self.refresh_size()?;
        match self.mode {
            RenderMode::AltScreen => self.enter_altscreen(),
            RenderMode::Inline => self.enter_inline(),
        }
    }

    pub fn exit(&mut self) -> io::Result<()> {
        self.refresh_size()?;
        match self.mode {
            RenderMode::AltScreen => self.exit_altscreen(),
            RenderMode::Inline => self.exit_inline(),
        }
    }

    fn enter_altscreen(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(())
    }

    fn enter_inline(&mut self) -> io::Result<()> {
        let (_, row) = position()?;
        let inline = self
            .inline_state
            .as_mut()
            .expect("inline_state must be Some");
        inline.block_start_row = row.min(self.state.size.height.saturating_sub(1));
        inline.last_rendered_block_start_row = inline.block_start_row;
        inline.last_cursor_row = 0;
        inline.last_skip = 0;
        terminal::enable_raw_mode()?;

        execute!(self.stdout, DisableLineWrap, Hide)?;
        Ok(())
    }

    fn exit_altscreen(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(
            self.stdout,
            DisableMouseCapture,
            LeaveAlternateScreen,
            EnableLineWrap,
            Show
        )?;

        if let Some(alt) = &self.alt_screen {
            let last_frame = alt.last_frame.clone();
            let width = self.state.size.width;
            self.print_frame_to_stdout(&last_frame, width)?;
        }
        self.stdout.flush()?;
        Ok(())
    }

    fn exit_inline(&mut self) -> io::Result<()> {
        let inline = self
            .inline_state
            .as_ref()
            .expect("inline_state must be Some");
        let max_row = self.state.size.height.saturating_sub(1);
        let block_start = inline.last_rendered_block_start_row.min(max_row);
        let last_row = if inline.last_drawn_count == 0 {
            block_start
        } else {
            block_start
                .saturating_add(inline.last_drawn_count.saturating_sub(1) as u16)
                .min(max_row)
        };

        queue!(self.stdout, MoveTo(0, last_row))?;
        execute!(self.stdout, EnableLineWrap, Show)?;
        terminal::disable_raw_mode()?;
        self.stdout.write_all(b"\r\n")?;
        self.stdout.flush()?;
        Ok(())
    }
}
