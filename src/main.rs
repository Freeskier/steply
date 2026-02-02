use std::io;
use std::time::Duration;
use steply::app::App;
use steply::terminal::Terminal;
use steply::terminal_event::TerminalEvent;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
    }
}

fn run() -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    terminal.enter_raw_mode()?;
    terminal.set_line_wrap(false)?;
    terminal.hide_cursor()?;

    let result = event_loop(&mut terminal);

    terminal.show_cursor()?;
    terminal.set_line_wrap(true)?;
    terminal.exit_raw_mode()?;

    result
}

fn event_loop(terminal: &mut Terminal) -> io::Result<()> {
    let mut app = App::new();

    let mut render_requested = true;

    loop {
        if terminal.poll(Duration::from_millis(100))? {
            match terminal.read_event()? {
                TerminalEvent::Key(key_event) => {
                    app.handle_key(key_event);
                    render_requested = true;
                }
                TerminalEvent::Resize { .. } => {
                    render_requested = true;
                }
            }
        }

        if app.tick() {
            render_requested = true;
        }

        if render_requested {
            app.render(terminal)?;
            render_requested = false;
        }

        if app.should_exit() {
            break;
        }
    }

    app.renderer.move_to_end(terminal)?;
    terminal.clear_from_cursor_down()?;

    Ok(())
}
