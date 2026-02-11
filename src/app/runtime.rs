use crate::app::command::{Command, map_key_to_command};
use crate::app::event::AppEvent;
use crate::app::scheduler::Scheduler;
use crate::domain::effect::Effect;
use crate::domain::reducer::Reducer;
use crate::state::app_state::AppState;
use crate::terminal::terminal::{Terminal, TerminalEvent};
use crate::ui::renderer::Renderer;
use std::io;
use std::time::{Duration, Instant};

pub struct Runtime {
    state: AppState,
    terminal: Terminal,
    scheduler: Scheduler,
}

impl Runtime {
    pub fn new(state: AppState, terminal: Terminal) -> Self {
        Self {
            state,
            terminal,
            scheduler: Scheduler::new(),
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.terminal.enter()?;

        let run_result = (|| -> io::Result<()> {
            self.render()?;

            while !self.state.should_exit {
                self.process_scheduled_events()?;

                let now = Instant::now();
                let timeout = self.scheduler.poll_timeout(now, Duration::from_millis(120));
                let event = self.terminal.poll_event(timeout)?;

                self.dispatch_app_event(AppEvent::Terminal(event))?;
            }

            Ok(())
        })();

        let exit_result = self.terminal.exit();
        run_result.and(exit_result)
    }

    fn process_scheduled_events(&mut self) -> io::Result<()> {
        for event in self.scheduler.drain_ready(Instant::now()) {
            self.dispatch_app_event(event)?;
        }
        Ok(())
    }

    fn dispatch_app_event(&mut self, event: AppEvent) -> io::Result<()> {
        match event {
            AppEvent::Terminal(TerminalEvent::Resize(size)) => {
                self.terminal.set_size(size);
                self.render()
            }
            AppEvent::Terminal(TerminalEvent::Key(key)) => {
                let command = map_key_to_command(key);
                self.process_command(command)
            }
            AppEvent::Terminal(TerminalEvent::Tick) => self.process_command(Command::Tick),
            AppEvent::Command(command) => self.process_command(command),
            AppEvent::Widget(widget_event) => {
                self.state.handle_widget_event(widget_event);
                self.render()
            }
        }
    }

    fn process_command(&mut self, command: Command) -> io::Result<()> {
        let effects = Reducer::reduce(&mut self.state, command);
        self.apply_effects(effects)
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> io::Result<()> {
        let mut render_requested = false;

        for effect in effects {
            match effect {
                Effect::EmitWidget(event) => {
                    self.state.handle_widget_event(event);
                    render_requested = true;
                }
                Effect::Schedule(cmd) => {
                    self.scheduler.schedule(cmd, Instant::now());
                }
                Effect::RequestRender => {
                    render_requested = true;
                }
            }
        }

        if render_requested {
            self.render()?;
        }

        Ok(())
    }

    fn render(&mut self) -> io::Result<()> {
        let frame = Renderer::render(&self.state, self.terminal.size());
        self.terminal.render(&frame.lines, frame.cursor)
    }
}
