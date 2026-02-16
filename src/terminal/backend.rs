use crate::ui::renderer::RenderFrame;
use crate::ui::span::SpanLine;
use crate::ui::style::Color;
use crossterm::cursor::{Hide, MoveTo, Show, position};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent,
    KeyModifiers as CrosstermKeyModifiers, MouseEventKind,
    DisableMouseCapture, EnableMouseCapture,
};
use crossterm::style::{
    Attribute, Color as CrosstermColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Stdout, Write};
use std::time::Duration;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Renders in the alternate screen buffer.  Supports scrolling through
    /// history during the session.  On exit, the full final frame is printed
    /// to the normal terminal buffer so it appears in scrollback history.
    #[default]
    AltScreen,
    /// Renders inline in the normal terminal buffer.  Done steps are committed
    /// once to scrollback and never re-rendered.  Only the active step is
    /// re-rendered in place.  Back navigation is disabled in this mode.
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Unknown,
    Char(char),
    Enter,
    Tab,
    BackTab,
    Esc,
    Backspace,
    Delete,
    Home,
    End,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Resize(TerminalSize),
    /// Mouse wheel scroll: positive = down, negative = up.
    Scroll(i32),
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPos {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalState {
    pub size: TerminalSize,
    pub cursor: Option<CursorPos>,
}

// ── Private mode-specific state ───────────────────────────────────────────────

struct AltScreenState {
    /// Index into the full frame that appears at the top of the viewport.
    scroll_offset: usize,
    /// When true the user has manually scrolled; suppress auto-follow-cursor
    /// until the next real user interaction resets this flag.
    manually_scrolled: bool,
    /// Cached last frame — printed to the normal buffer on exit.
    last_frame: Vec<SpanLine>,
}

impl AltScreenState {
    fn new() -> Self {
        Self {
            scroll_offset: 0,
            manually_scrolled: false,
            last_frame: Vec::new(),
        }
    }
}

struct InlineState {
    /// How many lines from the start of the frame have already been committed
    /// (printed once) to the terminal's scrollback buffer.
    committed_lines: usize,
}

impl InlineState {
    fn new() -> Self {
        Self { committed_lines: 0 }
    }
}

// ── Terminal ──────────────────────────────────────────────────────────────────

pub struct Terminal {
    stdout: Stdout,
    state: TerminalState,
    /// Inline mode: absolute terminal row where line 0 of the active frame is
    /// drawn.  Starts at the cursor row when `enter()` is called and can only
    /// decrease toward row 0.
    origin_row: u16,
    /// Number of active lines drawn in the previous render call (inline mode).
    last_rendered_lines: usize,
    mode: RenderMode,
    alt_screen: Option<AltScreenState>,
    inline_state: Option<InlineState>,
}

impl Terminal {
    pub fn new() -> io::Result<Self> {
        let (width, height) = terminal::size()?;
        Ok(Self {
            stdout: io::stdout(),
            state: TerminalState {
                size: TerminalSize { width, height },
                cursor: None,
            },
            origin_row: 0,
            last_rendered_lines: 0,
            mode: RenderMode::default(),
            alt_screen: Some(AltScreenState::new()), // default is AltScreen
            inline_state: None,
        })
    }

    /// Set the rendering mode.  Must be called before `enter()`.
    pub fn with_mode(mut self, mode: RenderMode) -> Self {
        self.mode = mode;
        self.alt_screen = if mode == RenderMode::AltScreen {
            Some(AltScreenState::new())
        } else {
            None
        };
        self.inline_state = if mode == RenderMode::Inline {
            Some(InlineState::new())
        } else {
            None
        };
        self
    }

    pub fn is_inline(&self) -> bool {
        self.mode == RenderMode::Inline
    }

    pub fn is_altscreen(&self) -> bool {
        self.mode == RenderMode::AltScreen
    }

    /// Scroll the viewport by `delta` lines (AltScreen only; no-op in Inline).
    /// Positive delta scrolls down (toward the end of the frame), negative up.
    pub fn scroll(&mut self, delta: i32) {
        let Some(alt) = &mut self.alt_screen else {
            return;
        };
        if delta != 0 {
            alt.manually_scrolled = true;
        }
        let new_offset = (alt.scroll_offset as i64 + delta as i64).max(0) as usize;
        alt.scroll_offset = new_offset;
    }

    /// Disable manual scroll — the next render call will auto-follow the cursor.
    pub fn reset_scroll(&mut self) {
        if let Some(alt) = &mut self.alt_screen {
            alt.manually_scrolled = false;
        }
    }

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

    /// Main render entry point — dispatches to the correct mode implementation.
    pub fn render_frame(&mut self, frame: &RenderFrame) -> io::Result<()> {
        self.refresh_size()?;
        self.state.cursor = frame.cursor;
        match self.mode {
            RenderMode::AltScreen => self.render_altscreen(frame),
            RenderMode::Inline => self.render_inline(frame),
        }
    }

    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<TerminalEvent> {
        if event::poll(timeout)? {
            match event::read()? {
                CrosstermEvent::Key(key) => Ok(TerminalEvent::Key(map_key_event(key))),
                CrosstermEvent::Resize(width, height) => {
                    Ok(TerminalEvent::Resize(TerminalSize { width, height }))
                }
                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => Ok(TerminalEvent::Scroll(-3)),
                    MouseEventKind::ScrollDown => Ok(TerminalEvent::Scroll(3)),
                    _ => Ok(TerminalEvent::Tick),
                },
                _ => Ok(TerminalEvent::Tick),
            }
        } else {
            Ok(TerminalEvent::Tick)
        }
    }

    pub fn size(&self) -> TerminalSize {
        self.state.size
    }

    pub fn state(&self) -> TerminalState {
        self.state
    }

    pub fn set_size(&mut self, size: TerminalSize) {
        self.state.size = size;
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let (width, height) = terminal::size()?;
        self.state.size = TerminalSize { width, height };
        Ok(())
    }
}

// ── Enter / Exit ──────────────────────────────────────────────────────────────

impl Terminal {
    fn enter_altscreen(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(())
    }

    fn enter_inline(&mut self) -> io::Result<()> {
        let (_, row) = position()?;
        self.origin_row = if self.state.size.height == 0 {
            0
        } else {
            row.saturating_add(1)
                .min(self.state.size.height.saturating_sub(1))
        };
        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnableMouseCapture, Hide)?;
        Ok(())
    }

    fn exit_altscreen(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(self.stdout, DisableMouseCapture, LeaveAlternateScreen, Show)?;
        // Print the last frame to the normal buffer so it appears in scrollback.
        if let Some(alt) = &self.alt_screen {
            let last_frame = alt.last_frame.clone();
            let width = self.state.size.width;
            self.print_frame_to_stdout(&last_frame, width)?;
        }
        self.stdout.flush()?;
        Ok(())
    }

    fn exit_inline(&mut self) -> io::Result<()> {
        let height = self.state.size.height;
        let rendered = self.last_rendered_lines as u16;
        let end_row = self
            .origin_row
            .saturating_add(rendered)
            .min(height.saturating_sub(1));

        if end_row < height.saturating_sub(1) {
            execute!(
                self.stdout,
                DisableMouseCapture,
                MoveTo(0, end_row),
                Clear(ClearType::CurrentLine),
                Show
            )?;
        } else {
            execute!(self.stdout, DisableMouseCapture, MoveTo(0, end_row), Show)?;
        }

        writeln!(self.stdout)?;
        self.stdout.flush()?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

// ── AltScreen rendering ───────────────────────────────────────────────────────

impl Terminal {
    fn render_altscreen(&mut self, frame: &RenderFrame) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        if height == 0 || width == 0 {
            return Ok(());
        }

        let frame_len = frame.lines.len();

        let alt = self.alt_screen.as_mut().expect("alt_screen must be Some in AltScreen mode");

        // Cache the full frame for printing on exit.
        alt.last_frame.clone_from(&frame.lines);

        let max_offset = frame_len.saturating_sub(height);

        // Auto-follow cursor unless the user has manually scrolled.
        if !alt.manually_scrolled {
            alt.scroll_offset = match frame.cursor {
                Some(cur) => {
                    // Keep cursor on the last row of the viewport (or higher).
                    (cur.row as usize).saturating_sub(height.saturating_sub(1))
                }
                None => max_offset,
            };
        }

        // Clamp to valid range.
        alt.scroll_offset = alt.scroll_offset.min(max_offset);
        let scroll_offset = alt.scroll_offset;

        // Clear and redraw.
        queue!(self.stdout, MoveTo(0, 0), Clear(ClearType::All))?;

        for row_idx in 0..height {
            let frame_line_idx = scroll_offset + row_idx;
            let Some(line) = frame.lines.get(frame_line_idx) else {
                break;
            };
            queue!(self.stdout, MoveTo(0, row_idx as u16))?;
            self.write_span_line(line, width)?;
        }

        // Position cursor.
        if let Some(cur) = frame.cursor {
            let frame_row = cur.row as usize;
            if frame_row >= scroll_offset {
                let screen_row = frame_row - scroll_offset;
                if screen_row < height {
                    let col = cur.col.min(width.saturating_sub(1));
                    queue!(self.stdout, MoveTo(col, screen_row as u16), Show)?;
                } else {
                    queue!(self.stdout, Hide)?;
                }
            } else {
                queue!(self.stdout, Hide)?;
            }
        } else {
            queue!(self.stdout, Hide)?;
        }

        self.stdout.flush()
    }
}

// ── Inline rendering ──────────────────────────────────────────────────────────

impl Terminal {
    fn render_inline(&mut self, frame: &RenderFrame) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        if height == 0 || width == 0 {
            return Ok(());
        }

        let inline = self
            .inline_state
            .as_mut()
            .expect("inline_state must be Some in Inline mode");
        let committed = inline.committed_lines;
        let frozen = frame.frozen_lines;

        // ── Step 1: clear the previous active render ───────────────────────
        for row_idx in 0..self.last_rendered_lines {
            let abs_row = self.origin_row as usize + row_idx;
            if abs_row >= height {
                break;
            }
            queue!(
                self.stdout,
                MoveTo(0, abs_row as u16),
                Clear(ClearType::CurrentLine)
            )?;
        }
        self.stdout.flush()?;

        // ── Step 2: commit newly frozen lines to scrollback ────────────────
        if frozen > committed {
            let new_lines = &frame.lines[committed..frozen];
            let n_new = new_lines.len();

            // Move to origin_row and print each newly-frozen line followed by
            // \r\n.  In raw mode \n alone only moves down without returning to
            // column 0, so we need \r\n.  The \n at the last row of the screen
            // causes the terminal to scroll, pushing content into scrollback.
            execute!(self.stdout, MoveTo(0, self.origin_row))?;
            for line in new_lines {
                self.write_span_line(line, width)?;
                // Clear to end of line so no stale characters remain if this
                // row was previously used by the active render.
                queue!(self.stdout, Clear(ClearType::UntilNewLine))?;
                self.stdout.write_all(b"\r\n")?;
            }
            self.stdout.flush()?;

            self.inline_state.as_mut().unwrap().committed_lines = frozen;
            // origin_row moves down by the number of lines we just printed.
            self.origin_row = (self.origin_row as usize + n_new)
                .min(height.saturating_sub(1)) as u16;
        }

        // ── Step 3: render active lines from origin_row ────────────────────
        let active_lines = &frame.lines[frozen..];
        let active_cursor = frame.cursor.map(|cur| CursorPos {
            row: cur.row.saturating_sub(frozen as u16),
            col: cur.col,
        });
        self.render_inline_active(active_lines, active_cursor)
    }

    /// Render a slice of lines starting at `origin_row`, scrolling the
    /// terminal if necessary to fit the content.  This is the core inline
    /// rendering engine used for the active step.
    fn render_inline_active(
        &mut self,
        lines: &[SpanLine],
        cursor: Option<CursorPos>,
    ) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        let frame_len = lines.len();

        // ── Make room by scrolling ─────────────────────────────────────────
        let available = height.saturating_sub(self.origin_row as usize);
        if frame_len > available {
            let need = frame_len.saturating_sub(available);
            let shift = need.min(self.origin_row as usize);
            if shift > 0 {
                execute!(self.stdout, MoveTo(0, (height - 1) as u16))?;
                for _ in 0..shift {
                    self.stdout.write_all(b"\n")?;
                }
                self.stdout.flush()?;
                self.origin_row = self.origin_row.saturating_sub(shift as u16);
            }
        }

        // ── Determine skip when pinned at row 0 ───────────────────────────
        let visible_rows = height.saturating_sub(self.origin_row as usize);
        let skip = frame_len.saturating_sub(visible_rows);
        let draw_count = frame_len.min(visible_rows);

        // ── Draw ──────────────────────────────────────────────────────────
        for row_idx in 0..draw_count {
            let abs_row = self.origin_row as usize + row_idx;
            if abs_row >= height {
                break;
            }
            queue!(
                self.stdout,
                MoveTo(0, abs_row as u16),
                Clear(ClearType::CurrentLine)
            )?;
            let Some(line) = lines.get(skip + row_idx) else {
                continue;
            };
            self.write_span_line(line, width)?;
        }

        self.last_rendered_lines = draw_count;

        // ── Position cursor ────────────────────────────────────────────────
        if let Some(cur) = cursor {
            let frame_row = cur.row as usize;
            if frame_row >= skip {
                let abs_row = self.origin_row as usize + (frame_row - skip);
                if abs_row < height {
                    let col = cur.col.min(width.saturating_sub(1));
                    queue!(self.stdout, MoveTo(col, abs_row as u16), Show)?;
                } else {
                    queue!(self.stdout, Hide)?;
                }
            } else {
                queue!(self.stdout, Hide)?;
            }
        } else {
            queue!(self.stdout, Hide)?;
        }

        self.stdout.flush()
    }
}

// ── Span rendering helpers ────────────────────────────────────────────────────

impl Terminal {
    /// Write a single `SpanLine` to stdout with ANSI styling, clipped to
    /// `width` columns.  Does NOT emit `MoveTo` or `Clear` — the caller is
    /// responsible for positioning the cursor first.
    fn write_span_line(&mut self, line: &SpanLine, width: u16) -> io::Result<()> {
        let mut used = 0usize;
        for span in line {
            if used >= width as usize {
                break;
            }
            let available_cols = (width as usize).saturating_sub(used);
            let clipped = clip_to_width(&span.text, available_cols);
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
            if span.style.strikethrough {
                queue!(self.stdout, SetAttribute(Attribute::CrossedOut))?;
            }
            queue!(self.stdout, Print(clipped.as_str()), ResetColor)?;
            if span.style.bold {
                queue!(self.stdout, SetAttribute(Attribute::NormalIntensity))?;
            }
            if span.style.strikethrough {
                queue!(self.stdout, SetAttribute(Attribute::NotCrossedOut))?;
            }
            used = used.saturating_add(UnicodeWidthStr::width(clipped.as_str()));
        }
        Ok(())
    }

    /// Print every line of `lines` to stdout as plain text (with ANSI styling)
    /// followed by `\n`.  Used to materialise the frame into the normal
    /// terminal buffer after leaving the alternate screen.
    fn print_frame_to_stdout(&mut self, lines: &[SpanLine], width: u16) -> io::Result<()> {
        for line in lines {
            self.write_span_line(line, width)?;
            self.stdout.write_all(b"\r\n")?;
        }
        Ok(())
    }
}

// ── Mapping functions ─────────────────────────────────────────────────────────

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

fn map_key_event(key: CrosstermKeyEvent) -> KeyEvent {
    KeyEvent {
        code: map_key_code(key.code),
        modifiers: map_key_modifiers(key.modifiers),
    }
}

fn map_key_code(code: CrosstermKeyCode) -> KeyCode {
    match code {
        CrosstermKeyCode::Char(ch) => KeyCode::Char(ch),
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => KeyCode::BackTab,
        CrosstermKeyCode::Esc => KeyCode::Esc,
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::PageUp => KeyCode::PageUp,
        CrosstermKeyCode::PageDown => KeyCode::PageDown,
        _ => KeyCode::Unknown,
    }
}

fn map_key_modifiers(modifiers: CrosstermKeyModifiers) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    if modifiers.contains(CrosstermKeyModifiers::SHIFT) {
        out.0 |= KeyModifiers::SHIFT.0;
    }
    if modifiers.contains(CrosstermKeyModifiers::CONTROL) {
        out.0 |= KeyModifiers::CONTROL.0;
    }
    if modifiers.contains(CrosstermKeyModifiers::ALT) {
        out.0 |= KeyModifiers::ALT.0;
    }
    out
}

fn clip_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut used = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(ch_width) > max_width {
            break;
        }
        out.push(ch);
        used = used.saturating_add(ch_width);
    }
    out
}
