use crate::ui::span::SpanLine;
use crate::ui::style::Color;
use crossterm::cursor::{Hide, MoveTo, Show, position};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent,
    KeyModifiers as CrosstermKeyModifiers,
};
use crossterm::style::{
    Color as CrosstermColor, Print, ResetColor, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, Stdout, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Unknown,
    Char(char),
    Enter,
    Tab,
    BackTab,
    Esc,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);

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

pub struct Terminal {
    stdout: Stdout,
    state: TerminalState,
    origin_row: u16,
    last_rendered_lines: usize,
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
        })
    }

    pub fn enter(&mut self) -> io::Result<()> {
        self.refresh_size()?;
        let (_, row) = position()?;
        self.origin_row = if self.state.size.height == 0 {
            0
        } else {
            row.saturating_add(1)
                .min(self.state.size.height.saturating_sub(1))
        };
        terminal::enable_raw_mode()?;
        execute!(self.stdout, Hide)?;
        Ok(())
    }

    pub fn exit(&mut self) -> io::Result<()> {
        self.refresh_size()?;
        let height = self.state.size.height;
        let max_rows = height.saturating_sub(self.origin_row) as usize;
        let used_rows = self.last_rendered_lines.min(max_rows) as u16;
        let final_row = if height == 0 {
            0
        } else {
            self.origin_row
                .saturating_add(used_rows)
                .min(height.saturating_sub(1))
        };
        execute!(
            self.stdout,
            MoveTo(0, final_row),
            Clear(ClearType::CurrentLine),
            Show
        )?;
        writeln!(self.stdout)?;
        self.stdout.flush()?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<TerminalEvent> {
        if event::poll(timeout)? {
            match event::read()? {
                CrosstermEvent::Key(key) => Ok(TerminalEvent::Key(map_key_event(key))),
                CrosstermEvent::Resize(width, height) => {
                    Ok(TerminalEvent::Resize(TerminalSize { width, height }))
                }
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

    pub fn render(&mut self, lines: &[SpanLine], cursor: Option<CursorPos>) -> io::Result<()> {
        self.refresh_size()?;
        self.state.cursor = cursor;

        let width = self.state.size.width;
        let height = self.state.size.height;
        let available_rows = height.saturating_sub(self.origin_row) as usize;
        let to_draw = lines.len().min(available_rows);
        let clear_rows = self.last_rendered_lines.max(to_draw);

        for row in 0..clear_rows {
            queue!(
                self.stdout,
                MoveTo(0, self.origin_row.saturating_add(row as u16)),
                Clear(ClearType::CurrentLine)
            )?;
            let Some(line) = lines.get(row) else {
                continue;
            };
            let mut used = 0usize;
            for span in line {
                if used >= width as usize {
                    break;
                }

                let available = (width as usize).saturating_sub(used);
                let clipped: String = span.text.chars().take(available).collect();
                if clipped.is_empty() {
                    continue;
                }

                if let Some(color) = span.style.color {
                    queue!(self.stdout, SetForegroundColor(map_color(color)))?;
                }
                if let Some(background) = span.style.background {
                    queue!(self.stdout, SetBackgroundColor(map_color(background)))?;
                }
                queue!(self.stdout, Print(clipped.clone()), ResetColor)?;
                used = used.saturating_add(clipped.chars().count());
            }
        }
        self.last_rendered_lines = to_draw;

        if let Some(cursor) = self.state.cursor {
            if width > 0 && height > 0 {
                let col = cursor.col.min(width.saturating_sub(1));
                let max_local_row = available_rows.saturating_sub(1) as u16;
                let row = self
                    .origin_row
                    .saturating_add(cursor.row.min(max_local_row))
                    .min(height.saturating_sub(1));
                queue!(self.stdout, MoveTo(col, row), Show)?;
            } else {
                queue!(self.stdout, Hide)?;
            }
        } else {
            queue!(self.stdout, Hide)?;
        }

        self.stdout.flush()
    }
}

fn map_color(color: Color) -> CrosstermColor {
    match color {
        Color::Reset => CrosstermColor::Reset,
        Color::Black => CrosstermColor::Black,
        Color::Red => CrosstermColor::DarkRed,
        Color::Green => CrosstermColor::DarkGreen,
        Color::Yellow => CrosstermColor::DarkYellow,
        Color::Blue => CrosstermColor::DarkBlue,
        Color::Magenta => CrosstermColor::DarkMagenta,
        Color::Cyan => CrosstermColor::DarkCyan,
        Color::White => CrosstermColor::White,
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
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
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
    out
}
