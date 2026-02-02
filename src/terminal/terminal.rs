use crate::frame::{Frame, Line};
use crate::style::Color;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::terminal_event::TerminalEvent;
use crossterm::event::{Event, KeyEventKind, poll, read};
use crossterm::style::{
    Attribute, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, Stdout, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

pub struct Terminal {
    stdout: Stdout,
    size: Size,
    cursor: Pos,
}

impl Terminal {
    pub fn new() -> io::Result<Self> {
        let stdout = io::stdout();
        let (width, height) = terminal::size()?;
        let (x, y) = cursor::position()?;
        Ok(Self {
            stdout,
            size: Size { width, height },
            cursor: Pos { x, y },
        })
    }

    pub fn writer_mut(&mut self) -> &mut Stdout {
        &mut self.stdout
    }

    pub fn enter_raw_mode(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    pub fn exit_raw_mode(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }

    pub fn set_line_wrap(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            execute!(self.stdout, terminal::EnableLineWrap)?;
        } else {
            execute!(self.stdout, terminal::DisableLineWrap)?;
        }
        Ok(())
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn cursor_position(&self) -> Pos {
        self.cursor
    }

    pub fn refresh_size(&mut self) -> io::Result<bool> {
        let (width, height) = terminal::size()?;
        let changed = self.size.width != width || self.size.height != height;
        self.size = Size { width, height };
        Ok(changed)
    }

    pub fn refresh_cursor_position(&mut self) -> io::Result<()> {
        let (x, y) = cursor::position()?;
        self.cursor = Pos { x, y };
        Ok(())
    }

    pub fn poll(&self, timeout: Duration) -> io::Result<bool> {
        poll(timeout)
    }

    pub fn read_event(&mut self) -> io::Result<TerminalEvent> {
        loop {
            let event = read()?;
            match event {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    return Ok(TerminalEvent::Key(map_key_event(key)));
                }
                Event::Resize(width, height) => {
                    self.size = Size { width, height };
                    return Ok(TerminalEvent::Resize { width, height });
                }
                _ => continue,
            }
        }
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Hide)?;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, cursor::Show)?;
        Ok(())
    }

    pub fn move_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        execute!(self.stdout, cursor::MoveTo(x, y))?;
        self.cursor = Pos { x, y };
        Ok(())
    }

    pub fn clear_line(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::CurrentLine)
        )?;
        Ok(())
    }

    pub fn clear_from_cursor_down(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )?;
        Ok(())
    }

    pub fn queue_hide_cursor(&mut self) -> io::Result<()> {
        queue!(self.stdout, cursor::Hide)?;
        Ok(())
    }

    pub fn queue_show_cursor(&mut self) -> io::Result<()> {
        queue!(self.stdout, cursor::Show)?;
        Ok(())
    }

    pub fn queue_move_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        queue!(self.stdout, cursor::MoveTo(x, y))?;
        self.cursor = Pos { x, y };
        Ok(())
    }

    pub fn queue_clear_line(&mut self) -> io::Result<()> {
        queue!(
            self.stdout,
            terminal::Clear(terminal::ClearType::CurrentLine)
        )?;
        Ok(())
    }

    pub fn render_line(&mut self, line: &Line) -> io::Result<()> {
        for span in line.spans() {
            let has_style = span.style().color().is_some()
                || span.style().background().is_some()
                || span.style().bold()
                || span.style().italic()
                || span.style().underline();

            if let Some(fg) = span.style().color() {
                write!(self.stdout, "{}", SetForegroundColor(map_color(fg)))?;
            }
            if let Some(bg) = span.style().background() {
                write!(self.stdout, "{}", SetBackgroundColor(map_color(bg)))?;
            }

            if span.style().bold() {
                write!(self.stdout, "{}", SetAttribute(Attribute::Bold))?;
            }
            if span.style().dim() {
                write!(self.stdout, "{}", SetAttribute(Attribute::Dim))?;
            }
            if span.style().italic() {
                write!(self.stdout, "{}", SetAttribute(Attribute::Italic))?;
            }
            if span.style().underline() {
                write!(self.stdout, "{}", SetAttribute(Attribute::Underlined))?;
            }

            write!(self.stdout, "{}", span.text())?;

            if has_style {
                write!(self.stdout, "{}", SetAttribute(Attribute::Reset))?;
                write!(self.stdout, "{}", ResetColor)?;
            }
        }
        Ok(())
    }

    pub fn render_frame(&mut self, frame: &Frame) -> io::Result<()> {
        for (i, line) in frame.lines().iter().enumerate() {
            if i > 0 {
                writeln!(self.stdout)?;
            }
            self.render_line(line)?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

fn map_color(color: Color) -> crossterm::style::Color {
    match color {
        Color::Black => crossterm::style::Color::Black,
        Color::DarkGrey => crossterm::style::Color::DarkGrey,
        Color::Red => crossterm::style::Color::Red,
        Color::Green => crossterm::style::Color::Green,
        Color::Yellow => crossterm::style::Color::Yellow,
        Color::Blue => crossterm::style::Color::Blue,
        Color::Magenta => crossterm::style::Color::Magenta,
        Color::Cyan => crossterm::style::Color::Cyan,
        Color::White => crossterm::style::Color::White,
    }
}

fn map_key_event(event: crossterm::event::KeyEvent) -> KeyEvent {
    KeyEvent {
        code: map_key_code(event.code),
        modifiers: map_key_modifiers(event.modifiers),
    }
}

fn map_key_code(code: crossterm::event::KeyCode) -> KeyCode {
    match code {
        crossterm::event::KeyCode::Char(ch) => KeyCode::Char(ch),
        crossterm::event::KeyCode::Backspace => KeyCode::Backspace,
        crossterm::event::KeyCode::Enter => KeyCode::Enter,
        crossterm::event::KeyCode::Esc => KeyCode::Esc,
        crossterm::event::KeyCode::Left => KeyCode::Left,
        crossterm::event::KeyCode::Right => KeyCode::Right,
        crossterm::event::KeyCode::Up => KeyCode::Up,
        crossterm::event::KeyCode::Down => KeyCode::Down,
        crossterm::event::KeyCode::Home => KeyCode::Home,
        crossterm::event::KeyCode::End => KeyCode::End,
        crossterm::event::KeyCode::Tab => KeyCode::Tab,
        crossterm::event::KeyCode::BackTab => KeyCode::BackTab,
        crossterm::event::KeyCode::Delete => KeyCode::Delete,
        _ => KeyCode::Other,
    }
}

fn map_key_modifiers(modifiers: crossterm::event::KeyModifiers) -> KeyModifiers {
    let mut mapped = KeyModifiers::NONE;
    if modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
        mapped |= KeyModifiers::SHIFT;
    }
    if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        mapped |= KeyModifiers::CONTROL;
    }
    if modifiers.contains(crossterm::event::KeyModifiers::ALT) {
        mapped |= KeyModifiers::ALT;
    }
    mapped
}
