use crate::ui::renderer::RenderFrame;
use crate::ui::span::SpanLine;
use crate::ui::style::{Color, Strike};
use crate::ui::text::{clip_to_display_width_without_linebreaks, text_display_width};
use crossterm::cursor::{Hide, MoveTo, Show, position};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, MouseEventKind,
};
use crossterm::style::{
    Attribute, Color as CrosstermColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen, ScrollUp,
};
use crossterm::{execute, queue};
use std::io::{self, Stdout, Write};
use std::time::Duration;

mod frame_diff;
mod input_mapping;
mod lifecycle;
mod rendering;
mod resize;
mod writer;

use frame_diff::{
    DirtyRows, compute_dirty_rows, estimate_self_reflow_cursor_delta, quick_frame_signature,
};
use input_mapping::{map_key_event, map_pointer_event};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    #[default]
    AltScreen,

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

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
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

    Scroll(i32),
    Pointer(PointerEvent),
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Move,
    Down(PointerButton),
    Up(PointerButton),
    Drag(PointerButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerSemantic {
    #[default]
    None,
    Filter,
    WrappedContinuation,
    Custom(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub col: u16,
    pub row: u16,
    pub modifiers: KeyModifiers,
    pub semantic: PointerSemantic,
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
    pub cursor_visible: bool,
}

struct AltScreenState {
    scroll_offset: usize,

    manually_scrolled: bool,

    last_frame: Vec<SpanLine>,
    last_frame_signature: u64,
    last_rendered_cursor: Option<CursorPos>,
    last_rendered_cursor_visible: bool,
    last_rendered_size: TerminalSize,
    last_rendered_scroll_offset: usize,
    has_rendered_once: bool,
}

impl AltScreenState {
    fn new() -> Self {
        Self {
            scroll_offset: 0,
            manually_scrolled: false,
            last_frame: Vec::new(),
            last_frame_signature: 0,
            last_rendered_cursor: None,
            last_rendered_cursor_visible: false,
            last_rendered_size: TerminalSize {
                width: 0,
                height: 0,
            },
            last_rendered_scroll_offset: 0,
            has_rendered_once: false,
        }
    }
}

struct InlineState {
    last_drawn_count: usize,

    last_cursor_row: u16,

    last_cursor_col: u16,

    block_start_row: u16,

    last_rendered_block_start_row: u16,

    last_frame: Vec<SpanLine>,
    last_frame_signature: u64,

    last_rendered_cursor: Option<CursorPos>,
    last_rendered_cursor_visible: bool,

    last_rendered_size: TerminalSize,

    has_rendered_once: bool,

    reanchor_after_resize: bool,

    last_resize_width_delta: i16,

    last_resize_height_delta: i16,
}

impl InlineState {
    fn new() -> Self {
        Self {
            last_drawn_count: 0,
            last_cursor_row: 0,
            last_cursor_col: 0,
            block_start_row: 0,
            last_rendered_block_start_row: 0,
            last_frame: Vec::new(),
            last_frame_signature: 0,
            last_rendered_cursor: None,
            last_rendered_cursor_visible: false,
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
                cursor_visible: false,
            },
            mode: RenderMode::default(),
            alt_screen: Some(AltScreenState::new()),
            inline_state: None,
        })
    }

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

    pub fn map_screen_row_to_frame_row(&self, screen_row: u16) -> u16 {
        match self.mode {
            RenderMode::AltScreen => {
                let offset = self
                    .alt_screen
                    .as_ref()
                    .map(|alt| alt.scroll_offset)
                    .unwrap_or(0);
                let row = offset.saturating_add(screen_row as usize);
                row.min(u16::MAX as usize) as u16
            }
            RenderMode::Inline => {
                let Some(inline) = self.inline_state.as_ref() else {
                    return screen_row;
                };
                let screen = screen_row as usize;
                let block_start = inline.last_rendered_block_start_row as usize;
                if screen < block_start {
                    return screen_row;
                }
                let visible_row = screen.saturating_sub(block_start);
                let skip = inline
                    .last_frame
                    .len()
                    .saturating_sub(inline.last_drawn_count);
                let row = skip.saturating_add(visible_row);
                row.min(u16::MAX as usize) as u16
            }
        }
    }

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

    pub fn reset_scroll(&mut self) {
        if let Some(alt) = &mut self.alt_screen {
            alt.manually_scrolled = false;
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
                    MouseEventKind::ScrollLeft => Ok(TerminalEvent::Scroll(-3)),
                    MouseEventKind::ScrollRight => Ok(TerminalEvent::Scroll(3)),
                    _ => Ok(map_pointer_event(mouse)
                        .map(TerminalEvent::Pointer)
                        .unwrap_or(TerminalEvent::Tick)),
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
        self.state.size = size;
        self.handle_inline_size_change(old, size);
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let old = self.state.size;
        let (width, height) = terminal::size()?;
        let new = TerminalSize { width, height };
        self.state.size = new;
        if old.width != width || old.height != height {
            self.handle_inline_size_change(old, new);
        }
        Ok(())
    }
}
