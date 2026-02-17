use crate::ui::renderer::RenderFrame;
use crate::ui::span::SpanLine;
use crate::ui::style::Color;
use crossterm::cursor::{Hide, MoveTo, Show, position};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent,
    KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent,
    KeyModifiers as CrosstermKeyModifiers, MouseEventKind,
};
use crossterm::style::{
    Attribute, Color as CrosstermColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::fs::OpenOptions;
use std::io::{self, Stdout, Write};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    /// Renders in the alternate screen buffer.  Supports scrolling through
    /// history during the session.  On exit, the full final frame is printed
    /// to the normal terminal buffer so it appears in scrollback history.
    #[default]
    AltScreen,
    /// Renders inline in the normal terminal buffer using deterministic block
    /// anchoring and explicit redraw planning.
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
    /// Number of lines drawn in the last render pass.
    last_drawn_count: usize,
    /// Last cursor row inside the rendered block (0-based).
    /// Kept so next renders can place the input cursor consistently after
    /// terminal size changes.
    last_cursor_row: u16,
    /// Last cursor column actually sent to terminal at the end of inline
    /// render. Used to estimate how terminal reflow moves the probe row.
    last_cursor_col: u16,
    /// Screen row where the first line of our block sits after the last render.
    /// Seeded from `position()` at `enter()`, then updated by inline layout
    /// planning on each render.
    block_start_row: u16,
    /// Cached last full frame for deterministic exit printing.
    last_frame: Vec<SpanLine>,
    /// Last cursor submitted to terminal for inline render.
    last_rendered_cursor: Option<CursorPos>,
    /// Last terminal size used for inline render.
    last_rendered_size: TerminalSize,
    /// Whether any inline frame has already been rendered.
    has_rendered_once: bool,
    /// Set after terminal resize; next render should re-anchor block start
    /// against real cursor row to follow terminal history reflow.
    reanchor_after_resize: bool,
    /// Last resize width delta (`new.width - old.width`) used to constrain
    /// re-anchor direction for pure width changes.
    last_resize_width_delta: i16,
    /// Last resize height delta (`new.height - old.height`).
    last_resize_height_delta: i16,
}

impl InlineState {
    fn new() -> Self {
        Self {
            last_drawn_count: 0,
            last_cursor_row: 0,
            last_cursor_col: 0,
            block_start_row: 0,
            last_frame: Vec::new(),
            last_rendered_cursor: None,
            last_rendered_size: TerminalSize {
                width: 0,
                height: 0,
            },
            has_rendered_once: false,
            reanchor_after_resize: false,
            last_resize_width_delta: 0,
            last_resize_height_delta: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InlineLayoutPlan {
    block_start_row: u16,
    clear_start_row: u16,
    draw_count: usize,
    skip: usize,
}

fn plan_inline_layout(
    height: usize,
    frame_len: usize,
    prev_block_start_row: u16,
) -> InlineLayoutPlan {
    if height == 0 {
        return InlineLayoutPlan {
            block_start_row: 0,
            clear_start_row: 0,
            draw_count: 0,
            skip: 0,
        };
    }

    let max_row = height.saturating_sub(1) as u16;
    let mut block_start = prev_block_start_row.min(max_row) as usize;
    let desired_visible = frame_len.min(height);
    let available = height.saturating_sub(block_start);

    if desired_visible > available {
        let need = desired_visible.saturating_sub(available);
        let shift_up = need.min(block_start);
        block_start = block_start.saturating_sub(shift_up);
    }

    let available_after_shift = height.saturating_sub(block_start);
    let draw_count = frame_len.min(available_after_shift);
    let skip = frame_len.saturating_sub(draw_count);
    let block_start_row = block_start.min(u16::MAX as usize) as u16;
    let clear_start_row = prev_block_start_row.min(block_start_row);

    InlineLayoutPlan {
        block_start_row,
        clear_start_row,
        draw_count,
        skip,
    }
}

// ── Terminal ──────────────────────────────────────────────────────────────────

pub struct Terminal {
    stdout: Stdout,
    state: TerminalState,
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
                    debug_log(format!("event resize {}x{}", width, height));
                    Ok(TerminalEvent::Resize(TerminalSize { width, height }))
                }
                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        debug_log("event mouse scroll up");
                        Ok(TerminalEvent::Scroll(-3))
                    }
                    MouseEventKind::ScrollDown => {
                        debug_log("event mouse scroll down");
                        Ok(TerminalEvent::Scroll(3))
                    }
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
        let old = self.state.size;
        debug_log(format!(
            "set_size {}x{} -> {}x{}",
            old.width, old.height, size.width, size.height
        ));
        self.state.size = size;
        self.handle_inline_size_change(old, size);
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let old = self.state.size;
        let (width, height) = terminal::size()?;
        let new = TerminalSize { width, height };
        self.state.size = new;
        if old.width != width || old.height != height {
            debug_log(format!(
                "refresh_size {}x{} -> {}x{}",
                old.width, old.height, width, height
            ));
            self.handle_inline_size_change(old, new);
        }
        Ok(())
    }

    fn handle_inline_size_change(&mut self, old: TerminalSize, new: TerminalSize) {
        if self.mode != RenderMode::Inline {
            return;
        }
        let Some(inline) = self.inline_state.as_mut() else {
            return;
        };
        if old == new {
            return;
        }

        if new.height == 0 {
            inline.block_start_row = 0;
            inline.last_cursor_row = 0;
            inline.reanchor_after_resize = false;
            inline.last_resize_width_delta = 0;
            inline.last_resize_height_delta = 0;
            return;
        }

        let max_row = new.height.saturating_sub(1);
        inline.block_start_row = inline.block_start_row.min(max_row);
        let max_cursor_row = max_row.saturating_sub(inline.block_start_row);
        inline.last_cursor_row = inline.last_cursor_row.min(max_cursor_row);
        inline.last_cursor_col = inline.last_cursor_col.min(new.width.saturating_sub(1));
        inline.reanchor_after_resize = true;
        inline.last_resize_width_delta = new.width as i16 - old.width as i16;
        inline.last_resize_height_delta = new.height as i16 - old.height as i16;
        debug_log(format!(
            "inline_resize old={}x{} new={}x{} block_start_row={} last_cursor_row={} last_cursor_col={} reanchor_after_resize={} dw={} dh={}",
            old.width,
            old.height,
            new.width,
            new.height,
            inline.block_start_row,
            inline.last_cursor_row,
            inline.last_cursor_col,
            inline.reanchor_after_resize,
            inline.last_resize_width_delta,
            inline.last_resize_height_delta
        ));
    }

    fn reanchor_inline_after_resize_if_needed(&mut self) {
        if self.mode != RenderMode::Inline {
            return;
        }
        if self.state.size.height == 0 {
            return;
        }
        let Some(inline_snapshot) = self.inline_state.as_ref().map(|inline| {
            (
                inline.reanchor_after_resize,
                inline.block_start_row,
                inline.last_cursor_row,
                inline.last_resize_width_delta,
                inline.last_resize_height_delta,
                estimate_self_reflow_cursor_delta(inline, self.state.size.width),
            )
        }) else {
            return;
        };

        let (pending, block_start_row, last_cursor_row, width_delta, height_delta, self_reflow_delta) =
            inline_snapshot;
        if !pending {
            return;
        }

        let max_row = self.state.size.height.saturating_sub(1);
        let expected_cursor_row = block_start_row.saturating_add(last_cursor_row).min(max_row);
        let maybe_actual_row = match position() {
            Ok((col, row)) => {
                debug_log(format!(
                    "inline_reanchor probe cursor=({},{}) expected_cursor_row={}",
                    col, row, expected_cursor_row
                ));
                Some(row.min(max_row))
            }
            Err(err) => {
                debug_log(format!(
                    "inline_reanchor probe failed expected_cursor_row={} err={}",
                    expected_cursor_row, err
                ));
                None
            }
        };

        let mut new_block_start_row = block_start_row;
        let mut delta = 0i32;
        if let Some(actual_row) = maybe_actual_row {
            let measured_delta = actual_row as i32 - expected_cursor_row as i32;
            // Remove movement caused by reflow of our own previously rendered
            // block. We only want drift originating from terminal history above
            // the block.
            delta = measured_delta - self_reflow_delta;
            debug_log(format!(
                "inline_reanchor delta measured={} self_reflow={} history={}",
                measured_delta, self_reflow_delta, delta
            ));
            // Guard against directional drift from noisy cursor probes:
            // for pure width expansion, block should not move down;
            // for pure width shrink, block should not move up.
            if height_delta == 0 {
                if width_delta > 0 && delta > 0 {
                    debug_log(format!(
                        "inline_reanchor clamp delta {} -> 0 (pure width expand dw={} dh={})",
                        delta, width_delta, height_delta
                    ));
                    delta = 0;
                } else if width_delta < 0 && delta < 0 {
                    debug_log(format!(
                        "inline_reanchor clamp delta {} -> 0 (pure width shrink dw={} dh={})",
                        delta, width_delta, height_delta
                    ));
                    delta = 0;
                }
            }
            if delta != 0 {
                new_block_start_row = (block_start_row as i32 + delta)
                    .clamp(0, max_row as i32) as u16;
            }
        }

        if let Some(inline) = self.inline_state.as_mut() {
            inline.reanchor_after_resize = false;
            inline.last_resize_width_delta = 0;
            inline.last_resize_height_delta = 0;
            inline.block_start_row = new_block_start_row;
            let max_cursor_row = max_row.saturating_sub(new_block_start_row);
            inline.last_cursor_row = inline.last_cursor_row.min(max_cursor_row);
            debug_log(format!(
                "inline_reanchor apply old_block_start_row={} new_block_start_row={} expected_cursor_row={} delta={} self_reflow_delta={} last_cursor_row={} dw={} dh={}",
                block_start_row,
                inline.block_start_row,
                expected_cursor_row,
                delta,
                self_reflow_delta,
                inline.last_cursor_row,
                width_delta,
                height_delta
            ));
        }
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
        // Query cursor position once at startup — the only DSR we ever do.
        // This tells us which screen row our block starts on.
        let (col, row) = position()?;
        let inline = self
            .inline_state
            .as_mut()
            .expect("inline_state must be Some");
        inline.block_start_row = row.min(self.state.size.height.saturating_sub(1));
        inline.last_cursor_row = 0;
        log_inline_environment();
        debug_log(format!(
            "enter_inline cursor_pos=({},{}) block_start_row={} terminal={}x{}",
            col, row, inline.block_start_row, self.state.size.width, self.state.size.height
        ));
        terminal::enable_raw_mode()?;
        // Prevent terminal soft-wrap from mutating physical row count of the
        // previously drawn block after resize. This keeps relative anchoring stable.
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
        let inline = self
            .inline_state
            .as_ref()
            .expect("inline_state must be Some");
        let last_frame = inline.last_frame.clone();
        let block_start = inline
            .block_start_row
            .min(self.state.size.height.saturating_sub(1));
        debug_log(format!(
            "exit_inline block_start={} last_drawn={} last_cursor_row={} last_frame_lines={} terminal={}x{}",
            block_start,
            inline.last_drawn_count,
            inline.last_cursor_row,
            last_frame.len(),
            self.state.size.width,
            self.state.size.height
        ));

        // Move to block start and clear everything below.
        queue!(
            self.stdout,
            MoveTo(0, block_start),
            Clear(ClearType::FromCursorDown)
        )?;

        // Print the last frame as permanent scrollback content.
        if !last_frame.is_empty() {
            self.print_frame_to_stdout(&last_frame, self.state.size.width)?;
        }

        execute!(self.stdout, EnableLineWrap, Show)?;
        terminal::disable_raw_mode()?;
        self.stdout.flush()?;
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

        let alt = self
            .alt_screen
            .as_mut()
            .expect("alt_screen must be Some in AltScreen mode");

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
    /// Render a frame inline by anchoring redraw to the tracked block start.
    ///
    /// This approach is resilient to terminal history reflow because:
    /// 1. Our lines are always printed at `width - 1` columns, so the terminal
    ///    never wraps them during reflow.
    /// 2. Layout is deterministic from terminal height + previous anchor, so
    ///    redraw does not depend on per-frame terminal cursor queries.
    /// 3. Synchronized output prevents tearing between clear and redraw.
    fn render_inline(&mut self, frame: &RenderFrame) -> io::Result<()> {
        let height = self.state.size.height as usize;
        let width = self.state.size.width;

        if height == 0 || width == 0 {
            return Ok(());
        }

        self.reanchor_inline_after_resize_if_needed();

        let (prev_block_start_row, prev_drawn, prev_cursor_row, skip_noop) = self
            .inline_state
            .as_ref()
            .map(|inline| {
                let same_frame = inline.last_frame == frame.lines;
                let same_cursor = inline.last_rendered_cursor == frame.cursor;
                let same_size = inline.last_rendered_size == self.state.size;
                let should_skip = inline.has_rendered_once
                    && !inline.reanchor_after_resize
                    && same_frame
                    && same_cursor
                    && same_size;
                (
                    inline.block_start_row,
                    inline.last_drawn_count,
                    inline.last_cursor_row,
                    should_skip,
                )
            })
            .unwrap_or((0, 0, 0, false));
        if skip_noop {
            debug_log("render_inline skip_noop");
            return Ok(());
        }
        if let Some(inline) = self.inline_state.as_mut() {
            inline.last_frame.clone_from(&frame.lines);
        }

        let frame_len = frame.lines.len();
        let plan = plan_inline_layout(height, frame_len, prev_block_start_row);
        let block_start_row = plan.block_start_row;
        let block_start = block_start_row as usize;
        let draw_count = plan.draw_count;
        let skip = plan.skip;
        let clear_start_row = plan.clear_start_row;

        debug_log(format!(
            "render_inline block_start={} clear_start={} prev_drawn={} prev_cursor_row={} frame_lines={} height={} width={} draw_count={} skip={} cursor={:?} frozen={}",
            block_start,
            clear_start_row,
            prev_drawn,
            prev_cursor_row,
            frame_len,
            height,
            width,
            draw_count,
            skip,
            frame.cursor,
            frame.frozen_lines
        ));

        queue!(self.stdout, BeginSynchronizedUpdate, Hide)?;
        queue!(
            self.stdout,
            MoveTo(0, clear_start_row),
            Clear(ClearType::FromCursorDown)
        )?;

        for visible_row in 0..draw_count {
            let target_row = block_start.saturating_add(visible_row) as u16;
            queue!(self.stdout, MoveTo(0, target_row))?;
            if let Some(line) = frame.lines.get(skip + visible_row) {
                self.write_span_line(line, width)?;
            }
        }

        let mut next_last_cursor_row = 0u16;
        let mut next_last_cursor_col = 0u16;

        if let Some(cursor) = frame.cursor {
            let cursor_row = cursor.row as usize;
            if cursor_row >= skip {
                let visible_row = cursor_row - skip;
                let target_row = block_start + visible_row;
                let col = cursor.col.min(width.saturating_sub(1));
                next_last_cursor_row = visible_row.min(u16::MAX as usize) as u16;
                next_last_cursor_col = col;
                debug_log(format!(
                    "render_inline cursor: frame_row={} visible_row={} target_row={} col={}",
                    cursor_row, visible_row, target_row, col
                ));
                queue!(self.stdout, MoveTo(col, target_row as u16), Show)?;
            } else {
                debug_log(format!(
                    "render_inline cursor hidden: frame_row={} < skip={}",
                    cursor_row, skip
                ));
                queue!(self.stdout, MoveTo(0, block_start_row), Hide)?;
            }
        } else {
            queue!(self.stdout, MoveTo(0, block_start_row), Hide)?;
        }

        if let Some(inline) = self.inline_state.as_mut() {
            inline.last_drawn_count = draw_count;
            inline.last_cursor_row = next_last_cursor_row;
            inline.last_cursor_col = next_last_cursor_col;
            inline.block_start_row = block_start_row;
            inline.last_rendered_cursor = frame.cursor;
            inline.last_rendered_size = self.state.size;
            inline.has_rendered_once = true;
        }

        queue!(self.stdout, EndSynchronizedUpdate)?;

        self.stdout.flush()
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

fn log_inline_environment() {
    if std::env::var_os("STEPLY_INLINE_DEBUG").is_none() {
        return;
    }
    let keys = [
        "STEPLY_TERM",
        "TERM",
        "COLORTERM",
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "TMUX",
        "ZELLIJ",
        "WEZTERM_PANE",
        "WT_SESSION",
        "KITTY_WINDOW_ID",
        "VTE_VERSION",
        "WAYLAND_DISPLAY",
        "DISPLAY",
    ];
    let mut parts = Vec::with_capacity(keys.len());
    for key in keys {
        let value = std::env::var(key).unwrap_or_else(|_| "-".to_string());
        parts.push(format!("{key}={value}"));
    }
    debug_log(format!("inline_env {}", parts.join(" ")));
}

fn debug_log(message: impl std::fmt::Display) {
    if std::env::var_os("STEPLY_INLINE_DEBUG").is_none() {
        return;
    }
    let log_path = std::env::var("STEPLY_INLINE_LOG")
        .unwrap_or_else(|_| format!("/tmp/steply-inline-{}.log", process::id()));
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(f, "[{}][pid={}] {}", ts_ms, process::id(), message);
    }
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

fn estimate_self_reflow_cursor_delta(inline: &InlineState, new_width: u16) -> i32 {
    if !inline.has_rendered_once || inline.last_drawn_count == 0 {
        return 0;
    }
    let old_width = inline.last_rendered_size.width;
    if old_width == 0 || new_width == 0 {
        return 0;
    }

    let old_skip = inline.last_frame.len().saturating_sub(inline.last_drawn_count);
    let visible_lines = &inline.last_frame[old_skip..];
    if visible_lines.is_empty() {
        return 0;
    }

    let cursor = match inline.last_rendered_cursor {
        Some(cursor) => cursor,
        None => return 0,
    };
    let cursor_abs_row = cursor.row as usize;
    if cursor_abs_row < old_skip {
        return 0;
    }
    let cursor_visible_row = inline
        .last_cursor_row
        .min(visible_lines.len().saturating_sub(1) as u16) as usize;

    let new_width_usize = new_width as usize;
    let mut new_row = 0usize;
    for line in visible_lines.iter().take(cursor_visible_row) {
        let width = rendered_line_width(line, old_width);
        new_row = new_row.saturating_add(wrapped_rows(width, new_width_usize));
    }

    if visible_lines.get(cursor_visible_row).is_none() {
        return 0;
    }
    // Use the actual cursor column that was moved to during the last render,
    // not the raw frame cursor. This mirrors the probe row that `position()`
    // measures after resize.
    let prefix = inline
        .last_cursor_col
        .min(old_width.saturating_sub(1)) as usize;
    new_row = new_row.saturating_add(prefix / new_width_usize);

    new_row as i32 - cursor_visible_row as i32
}

fn rendered_line_width(line: &SpanLine, old_width: u16) -> usize {
    let render_width = if old_width > 1 {
        (old_width - 1) as usize
    } else {
        old_width as usize
    };
    if render_width == 0 {
        return 0;
    }

    let mut used = 0usize;
    for span in line {
        if used >= render_width {
            break;
        }
        let available = render_width.saturating_sub(used);
        let mut span_used = 0usize;
        for ch in span.text.chars().filter(|ch| !matches!(ch, '\n' | '\r')) {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if span_used.saturating_add(ch_width) > available {
                break;
            }
            span_used = span_used.saturating_add(ch_width);
        }
        used = used.saturating_add(span_used);
    }
    used
}

fn wrapped_rows(line_width: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if line_width == 0 {
        return 1;
    }
    (line_width.saturating_sub(1) / width).saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::{
        CursorPos, InlineState, TerminalSize, estimate_self_reflow_cursor_delta, plan_inline_layout,
    };
    use crate::ui::span::{Span, SpanLine};

    fn line_with_width(width: usize) -> SpanLine {
        vec![Span::new("x".repeat(width))]
    }

    #[test]
    fn inline_layout_keeps_anchor_when_frame_fits() {
        let plan = plan_inline_layout(24, 8, 10);
        assert_eq!(plan.block_start_row, 10);
        assert_eq!(plan.clear_start_row, 10);
        assert_eq!(plan.draw_count, 8);
        assert_eq!(plan.skip, 0);
    }

    #[test]
    fn inline_layout_shifts_up_when_not_enough_space() {
        let plan = plan_inline_layout(24, 10, 18);
        assert_eq!(plan.block_start_row, 14);
        assert_eq!(plan.clear_start_row, 14);
        assert_eq!(plan.draw_count, 10);
        assert_eq!(plan.skip, 0);
    }

    #[test]
    fn inline_layout_shows_tail_when_frame_is_taller_than_terminal() {
        let plan = plan_inline_layout(24, 40, 5);
        assert_eq!(plan.block_start_row, 0);
        assert_eq!(plan.clear_start_row, 0);
        assert_eq!(plan.draw_count, 24);
        assert_eq!(plan.skip, 16);
    }

    #[test]
    fn inline_layout_clamps_out_of_range_anchor() {
        let plan = plan_inline_layout(10, 3, 20);
        assert_eq!(plan.block_start_row, 7);
        assert_eq!(plan.clear_start_row, 7);
        assert_eq!(plan.draw_count, 3);
        assert_eq!(plan.skip, 0);
    }

    #[test]
    fn self_reflow_delta_counts_new_wrap_rows_before_cursor() {
        let mut inline = InlineState::new();
        inline.has_rendered_once = true;
        inline.last_drawn_count = 5;
        inline.last_cursor_row = 4;
        inline.last_cursor_col = 14;
        inline.last_rendered_size = TerminalSize {
            width: 83,
            height: 24,
        };
        inline.last_rendered_cursor = Some(CursorPos { col: 14, row: 4 });
        inline.last_frame = vec![
            line_with_width(1),
            line_with_width(82),
            line_with_width(1),
            line_with_width(1),
            line_with_width(12),
        ];

        // Shrink 83 -> 81. The second line (82 cols) gains one wrapped row.
        assert_eq!(estimate_self_reflow_cursor_delta(&inline, 81), 1);
    }

    #[test]
    fn self_reflow_delta_uses_actual_rendered_cursor_col() {
        let mut inline = InlineState::new();
        inline.has_rendered_once = true;
        inline.last_drawn_count = 3;
        inline.last_cursor_row = 2;
        inline.last_cursor_col = 10;
        inline.last_rendered_size = TerminalSize {
            width: 40,
            height: 24,
        };
        // Raw frame cursor can be far to the right, but probe is at
        // `last_cursor_col` (actual MoveTo col used in render pass).
        inline.last_rendered_cursor = Some(CursorPos { col: 70, row: 2 });
        inline.last_frame = vec![line_with_width(1), line_with_width(1), line_with_width(1)];

        assert_eq!(estimate_self_reflow_cursor_delta(&inline, 8), 1);
    }

    #[test]
    fn self_reflow_delta_is_zero_when_cursor_not_in_visible_tail() {
        let mut inline = InlineState::new();
        inline.has_rendered_once = true;
        inline.last_drawn_count = 3;
        inline.last_cursor_row = 0;
        inline.last_cursor_col = 0;
        inline.last_rendered_size = TerminalSize {
            width: 80,
            height: 24,
        };
        // Cursor was above visible tail, so the probe sat at block start.
        inline.last_rendered_cursor = Some(CursorPos { col: 0, row: 0 });
        inline.last_frame = vec![
            line_with_width(10),
            line_with_width(10),
            line_with_width(10),
            line_with_width(10),
            line_with_width(10),
        ];

        assert_eq!(estimate_self_reflow_cursor_delta(&inline, 60), 0);
    }
}
