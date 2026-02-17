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
use crossterm::terminal::{
    self, Clear, ClearType, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::fs::OpenOptions;
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
    /// Width reflow detected; adjust anchor once at next inline render.
    reflow_pending: bool,
    /// Cached last full frame for deterministic inline exit printing.
    last_frame: Vec<SpanLine>,
}

impl InlineState {
    fn new() -> Self {
        Self {
            reflow_pending: false,
            last_frame: Vec::new(),
        }
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
    /// Inline mode: absolute row where the previous active frame started.
    /// Used to clear exactly the region that was rendered last time.
    last_render_origin_row: u16,
    /// Last known terminal cursor row after our render pass.
    /// Used to detect row shifts caused by terminal reflow on width resize.
    last_known_cursor_row: Option<u16>,
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
            last_render_origin_row: 0,
            last_known_cursor_row: None,
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
        let width_changed = self.state.size.width != size.width;
        let old = self.state.size;
        self.state.size = size;
        debug_log(format!(
            "set_size old={}x{} new={}x{} width_changed={}",
            old.width, old.height, size.width, size.height, width_changed
        ));
        if width_changed {
            self.mark_inline_width_change();
        }
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let old_width = self.state.size.width;
        let old_height = self.state.size.height;
        let (width, height) = terminal::size()?;
        self.state.size = TerminalSize { width, height };
        debug_log(format!(
            "refresh_size old={}x{} new={}x{} width_changed={}",
            old_width,
            old_height,
            width,
            height,
            old_width != width
        ));
        if old_width != width {
            self.mark_inline_width_change();
        }
        Ok(())
    }

    fn mark_inline_width_change(&mut self) {
        if self.mode != RenderMode::Inline {
            return;
        }
        if let Some(inline) = self.inline_state.as_mut() {
            inline.reflow_pending = true;
            debug_log(format!("mark_inline_width_change reflow_pending={}", inline.reflow_pending));
        }
    }

    fn adjust_inline_anchor_for_reflow(&mut self) {
        if self.mode != RenderMode::Inline {
            return;
        }
        let Some(prev_row) = self.last_known_cursor_row else {
            return;
        };
        let Ok((_, current_row)) = position() else {
            return;
        };
        let delta = current_row as i32 - prev_row as i32;
        debug_log(format!(
            "adjust_inline_anchor_for_reflow prev_row={} current_row={} delta={} before_origin={} before_last_origin={}",
            prev_row, current_row, delta, self.origin_row, self.last_render_origin_row
        ));
        if delta == 0 {
            return;
        }
        let max_row = self.state.size.height.saturating_sub(1) as i32;
        self.origin_row = (self.origin_row as i32 + delta).clamp(0, max_row) as u16;
        self.last_render_origin_row =
            (self.last_render_origin_row as i32 + delta).clamp(0, max_row) as u16;
        self.last_known_cursor_row = Some(current_row);
        debug_log(format!(
            "adjust_inline_anchor_for_reflow after_origin={} after_last_origin={} max_row={}",
            self.origin_row, self.last_render_origin_row, max_row
        ));
    }

}

// ── Enter / Exit ──────────────────────────────────────────────────────────────

impl Terminal {
    fn enter_altscreen(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            self.stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            Hide
        )?;
        Ok(())
    }

    fn enter_inline(&mut self) -> io::Result<()> {
        let (_, row) = position()?;
        self.origin_row = if self.state.size.height == 0 {
            0
        } else {
            row.min(self.state.size.height.saturating_sub(1))
        };
        terminal::enable_raw_mode()?;
        execute!(self.stdout, Hide)?;
        self.last_render_origin_row = self.origin_row;
        self.last_known_cursor_row = None;
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
        let last_frame = self
            .inline_state
            .as_ref()
            .map(|s| s.last_frame.clone())
            .unwrap_or_default();
        let anchor_row = self
            .last_render_origin_row
            .min(self.state.size.height.saturating_sub(1));
        let height = self.state.size.height;
        let rendered = self.last_rendered_lines as u16;
        let clear_start = self.last_render_origin_row.min(self.origin_row);
        for row in clear_start..height {
            queue!(self.stdout, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
        }

        let end_row = self
            .last_render_origin_row
            .saturating_add(rendered)
            .min(height.saturating_sub(1));

        if end_row < height.saturating_sub(1) {
            execute!(
                self.stdout,
                MoveTo(0, end_row),
                Clear(ClearType::CurrentLine),
                EnableLineWrap,
                Show
            )?;
        } else {
            execute!(self.stdout, MoveTo(0, end_row), EnableLineWrap, Show)?;
        }

        terminal::disable_raw_mode()?;
        if !last_frame.is_empty() {
            execute!(self.stdout, MoveTo(0, anchor_row))?;
            self.print_frame_to_stdout(last_frame.as_slice(), self.state.size.width)?;
        }
        writeln!(self.stdout)?;
        self.stdout.flush()?;
        self.last_known_cursor_row = None;
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

        let mut reflow_pending = self
            .inline_state
            .as_ref()
            .expect("inline_state must be Some in Inline mode")
            .reflow_pending;

        debug_log(format!(
            "render_inline start lines={} frozen={} origin_row={} last_render_origin_row={} reflow_pending={} cursor={:?}",
            frame.lines.len(),
            frame.frozen_lines,
            self.origin_row,
            self.last_render_origin_row,
            reflow_pending,
            frame.cursor
        ));

        if reflow_pending {
            debug_log("render_inline apply_reflow_anchor (tracking-row based)");
            self.adjust_inline_anchor_for_reflow();
            reflow_pending = false;
        }

        self.origin_row = self
            .origin_row
            .min(self.state.size.height.saturating_sub(1));

        let inline = self.inline_state.as_mut().unwrap();
        inline.last_frame.clone_from(&frame.lines);
        inline.reflow_pending = reflow_pending;
        debug_log(format!(
            "render_inline before_active active_lines={} origin_row={} reflow_pending={}",
            frame.lines.len(),
            self.origin_row,
            inline.reflow_pending
        ));

        self.render_inline_active(&frame.lines, frame.cursor)
    }

    /// Render a slice of lines starting at `origin_row`.
    ///
    /// Inline mode keeps history by committing frozen lines once. The active
    /// viewport is then redrawn deterministically from `origin_row` down to
    /// the bottom of the terminal.
    fn render_inline_active(
        &mut self,
        lines: &[SpanLine],
        cursor: Option<CursorPos>,
    ) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        let frame_len = lines.len();
        let mut draw_origin = self.origin_row.min(self.state.size.height.saturating_sub(1));
        debug_log(format!(
            "render_inline_active start frame_len={} height={} width={} draw_origin={} cursor={:?}",
            frame_len, height, width, draw_origin, cursor
        ));

        // If there is not enough room below origin_row, clamp render origin
        // upward instead of forcing terminal scroll with trailing newlines.
        // This avoids bottom-edge artifacts when resizing near shell prompt.
        let desired_visible = frame_len.min(height);
        let available = height.saturating_sub(draw_origin as usize);
        if desired_visible > available {
            let need = desired_visible.saturating_sub(available);
            debug_log(format!(
                "render_inline_active clamp need={} desired_visible={} available={} draw_origin_before={}",
                need, desired_visible, available, draw_origin
            ));
            draw_origin = draw_origin.saturating_sub(need as u16);
            self.origin_row = draw_origin;
            debug_log(format!(
                "render_inline_active clamp done draw_origin_after={} origin_row={}",
                draw_origin, self.origin_row
            ));
        }

        let visible_rows = height.saturating_sub(draw_origin as usize);
        let skip = frame_len.saturating_sub(visible_rows);
        let draw_count = frame_len.min(visible_rows);

        // Clear the entire active viewport tail to avoid stale artifacts.
        for abs_row in draw_origin as usize..height {
            queue!(
                self.stdout,
                MoveTo(0, abs_row as u16),
                Clear(ClearType::CurrentLine)
            )?;
        }
        for row_idx in 0..draw_count {
            let abs_row = draw_origin as usize + row_idx;
            if abs_row >= height {
                break;
            }
            queue!(self.stdout, MoveTo(0, abs_row as u16))?;
            let Some(line) = lines.get(skip + row_idx) else {
                continue;
            };
            self.write_span_line(line, width)?;
        }

        self.last_rendered_lines = draw_count;
        self.last_render_origin_row = draw_origin;

        // ── Position technical tracking cursor ─────────────────────────────
        // Park cursor at the first row of inline block (never on shell prompt
        // line above), so history reflow still shifts it but prompt is untouched.
        let tracking_row = Some(draw_origin);
        queue!(self.stdout, MoveTo(0, draw_origin), Hide)?;

        self.stdout.flush()?;
        self.last_known_cursor_row = tracking_row;
        debug_log(format!(
            "render_inline_active end draw_origin={} draw_count={} tracking_row={:?} logical_cursor={:?} last_known_cursor_row={:?}",
            draw_origin, draw_count, tracking_row, cursor, self.last_known_cursor_row
        ));
        Ok(())
    }
}

// ── Span rendering helpers ────────────────────────────────────────────────────

impl Terminal {
    /// Write a single `SpanLine` to stdout with ANSI styling, clipped to
    /// `width` columns.  Does NOT emit `MoveTo` or `Clear` — the caller is
    /// responsible for positioning the cursor first.
    fn write_span_line(&mut self, line: &SpanLine, width: u16) -> io::Result<()> {
        self.write_span_line_with_margin(line, width, true)
    }

    fn write_span_line_with_margin(
        &mut self,
        line: &SpanLine,
        width: u16,
        keep_one_cell_margin: bool,
    ) -> io::Result<()> {
        // Keep one-cell safety margin in raw inline rendering to avoid
        // terminal-specific auto-wrap/reflow when printing at the last column.
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
            // Final transcript print should not be clipped by the inline
            // safety margin; let terminal line-wrap rules apply normally.
            self.write_span_line_with_margin(line, width, false)?;
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
    for ch in text.chars().filter(|ch| !matches!(ch, '\n' | '\r')) {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(ch_width) > max_width {
            break;
        }
        out.push(ch);
        used = used.saturating_add(ch_width);
    }
    out
}

fn debug_log(message: impl AsRef<str>) {
    if std::env::var_os("STEPLY_INLINE_DEBUG").is_none() {
        return;
    }
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/steply-inline.log")
    {
        let _ = writeln!(f, "{}", message.as_ref());
    }
}
