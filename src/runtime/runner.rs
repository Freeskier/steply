use crate::runtime::effect::Effect;
use crate::runtime::event::{AppEvent, SystemEvent, WidgetAction};
use crate::runtime::intent::Intent;
use crate::runtime::key_bindings::KeyBindings;
use crate::runtime::reducer::Reducer;
use crate::runtime::scheduler::Scheduler;
use crate::state::app::AppState;
use crate::task::{LogLine, TaskExecutor};
use crate::terminal::{Terminal, TerminalEvent};
use crate::ui::render_view::RenderView;
use crate::ui::renderer::{Renderer, RendererConfig};
use std::io;
use std::time::{Duration, Instant};

pub struct Runtime {
    state: AppState,
    terminal: Terminal,
    scheduler: Scheduler,
    task_executor: TaskExecutor,
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
            task_executor: TaskExecutor::new(),
            key_bindings,
            renderer,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.terminal.enter()?;

        let run_result = (|| -> io::Result<()> {
            self.flush_pending_task_invocations();
            self.render()?;

            while !self.state.should_exit() {
                self.process_scheduled_events()?;
                self.process_task_log_lines()?;
                self.process_task_completions()?;
                self.flush_pending_task_invocations();

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

    fn process_task_log_lines(&mut self) -> io::Result<()> {
        for LogLine { task_id, line } in self.task_executor.drain_log_lines() {
            self.dispatch_app_event(AppEvent::System(SystemEvent::TaskLogLine { task_id, line }))?;
        }
        Ok(())
    }

    fn process_task_completions(&mut self) -> io::Result<()> {
        for completion in self.task_executor.drain_ready() {
            self.dispatch_app_event(AppEvent::System(SystemEvent::TaskCompleted { completion }))?;
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
                let intent = self
                    .key_bindings
                    .resolve(key)
                    .unwrap_or(Intent::InputKey(key));
                self.process_intent(intent)
            }
            AppEvent::Terminal(TerminalEvent::Tick) => self.process_intent(Intent::Tick),
            AppEvent::Intent(intent) => self.process_intent(intent),
            AppEvent::Action(action) => {
                if self.apply_action(action) {
                    self.render()?;
                }
                Ok(())
            }
            AppEvent::System(event) => {
                if self.apply_system_event(event) {
                    self.render()?;
                }
                Ok(())
            }
        }
    }

    fn process_intent(&mut self, intent: Intent) -> io::Result<()> {
        let effects = Reducer::reduce(&mut self.state, intent);
        self.apply_effects(effects)
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> io::Result<()> {
        let mut render_requested = false;

        for effect in effects {
            match effect {
                Effect::Action(action) => {
                    render_requested |= self.apply_action(action);
                }
                Effect::System(event) => {
                    render_requested |= self.apply_system_event(event);
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

    fn apply_action(&mut self, action: WidgetAction) -> bool {
        let result = self.state.handle_action(action);
        self.flush_pending_task_invocations();
        result.request_render
    }

    fn apply_system_event(&mut self, event: SystemEvent) -> bool {
        let result = self.state.handle_system_event(event);
        self.flush_pending_task_invocations();
        result.request_render
    }

    fn flush_pending_task_invocations(&mut self) {
        for invocation in self.state.take_pending_task_invocations() {
            self.task_executor.spawn(invocation);
        }
    }

    fn render(&mut self) -> io::Result<()> {
        let view = RenderView::from_state(&self.state);
        let frame = self.renderer.render(&view, self.terminal.size());
        self.terminal.render(&frame.lines, frame.cursor)
    }
}
