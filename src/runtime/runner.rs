use crate::core::effect::Effect;
use crate::core::reducer::Reducer;
use crate::runtime::command::Command;
use crate::runtime::event::{AppEvent, WidgetEvent};
use crate::runtime::key_bindings::KeyBindings;
use crate::runtime::scheduler::Scheduler;
use crate::state::app_state::AppState;
use crate::terminal::{Terminal, TerminalEvent};
use crate::ui::renderer::{Renderer, RendererConfig};
use std::io;
use std::time::{Duration, Instant};

pub struct Runtime {
    state: AppState,
    terminal: Terminal,
    scheduler: Scheduler,
    key_bindings: KeyBindings,
    renderer: Renderer,
}

impl Runtime {
    pub fn new(state: AppState, terminal: Terminal) -> Self {
        Self::with_parts(state, terminal, KeyBindings::new(), Renderer::default())
    }

    pub fn with_key_bindings(
        state: AppState,
        terminal: Terminal,
        key_bindings: KeyBindings,
    ) -> Self {
        Self::with_parts(state, terminal, key_bindings, Renderer::default())
    }

    pub fn with_renderer(mut self, renderer: Renderer) -> Self {
        self.renderer = renderer;
        self
    }

    pub fn with_renderer_config(mut self, config: RendererConfig) -> Self {
        self.renderer = Renderer::new(config);
        self
    }

    fn with_parts(
        state: AppState,
        terminal: Terminal,
        key_bindings: KeyBindings,
        renderer: Renderer,
    ) -> Self {
        Self {
            state,
            terminal,
            scheduler: Scheduler::new(),
            key_bindings,
            renderer,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.terminal.enter()?;

        let run_result = (|| -> io::Result<()> {
            self.render()?;

            while !self.state.should_exit() {
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
                let command = self
                    .key_bindings
                    .resolve(key)
                    .unwrap_or(Command::InputKey(key));
                self.process_command(command)
            }
            AppEvent::Terminal(TerminalEvent::Tick) => self.process_command(Command::Tick),
            AppEvent::Command(command) => self.process_command(command),
            AppEvent::Widget(widget_event) => {
                if self.apply_widget_event(widget_event) {
                    self.render()?;
                }
                Ok(())
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
                    render_requested |= self.apply_widget_event(event);
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

    fn apply_widget_event(&mut self, event: WidgetEvent) -> bool {
        self.state.handle_widget_event(event)
    }

    fn render(&mut self) -> io::Result<()> {
        let frame = self.renderer.render(&self.state, self.terminal.size());
        self.terminal.render(&frame.lines, frame.cursor)
    }
}
