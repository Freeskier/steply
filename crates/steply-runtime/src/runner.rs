use crate::clipboard;
use crate::selection::{
    SelectionState, apply_selection_highlight, extract_selected_text, handle_selection_pointer,
};
use crate::task_executor::{LogLine, TaskExecutor};
use std::io;
use std::time::{Duration, Instant};
use steply_core::preview::render::render_json as render_preview_json;
use steply_core::preview::request::RenderJsonRequest;
use steply_core::runtime::effect::Effect;
use steply_core::runtime::event::{AppEvent, SystemEvent, WidgetAction};
use steply_core::runtime::intent::Intent;
use steply_core::runtime::key_bindings::KeyBindings;
use steply_core::runtime::reducer::Reducer;
use steply_core::runtime::scheduler::Scheduler;
use steply_core::state::app::AppState;
use steply_core::terminal::TerminalEvent;
use steply_core::ui::hit_test::FrameHitMap;
use steply_core::ui::render_view::RenderView;
use steply_core::ui::renderer::{Renderer, RendererConfig};
use steply_core::ui::span::SpanLine;

use crate::terminal::{RenderMode, Terminal};

pub struct Runtime {
    state: AppState,
    terminal: Terminal,
    scheduler: Scheduler,
    task_executor: TaskExecutor,
    key_bindings: KeyBindings,
    renderer: Renderer,
    last_hit_map: FrameHitMap,
    selection: SelectionState,
    last_frame_lines: Vec<SpanLine>,
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

    pub fn with_render_mode(mut self, mode: RenderMode) -> Self {
        self.terminal = self.terminal.with_mode(mode);
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
            last_hit_map: FrameHitMap::default(),
            selection: SelectionState::default(),
            last_frame_lines: Vec::new(),
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

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn into_state(self) -> AppState {
        self.state
    }

    pub fn print_render_json(&mut self) -> io::Result<()> {
        self.print_render_json_with_request(RenderJsonRequest::default())
    }

    pub fn print_render_json_with_request(&mut self, request: RenderJsonRequest) -> io::Result<()> {
        let size = self.terminal.size();
        let doc = render_preview_json(&mut self.state, &request, &mut self.renderer, size)
            .map_err(io::Error::other)?;
        let json = serde_json::to_string_pretty(&doc)
            .map_err(|err| io::Error::other(format!("failed to encode render json: {err}")))?;
        println!("{json}");
        Ok(())
    }

    fn process_scheduled_events(&mut self) -> io::Result<()> {
        for event in self.scheduler.drain_ready(Instant::now()) {
            self.dispatch_app_event(event)?;
        }
        Ok(())
    }

    fn process_task_log_lines(&mut self) -> io::Result<()> {
        for LogLine {
            task_id,
            run_id,
            line,
        } in self.task_executor.drain_log_lines()
        {
            self.dispatch_app_event(AppEvent::System(SystemEvent::TaskLogLine {
                task_id,
                run_id,
                line,
            }))?;
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
            AppEvent::Terminal(TerminalEvent::Scroll(delta)) => {
                self.terminal.scroll(delta);
                self.render()
            }
            AppEvent::Terminal(TerminalEvent::Pointer(event)) => {
                let mut frame_event = event;
                frame_event.row = self.terminal.map_screen_row_to_frame_row(event.row);

                let (selection_consumed, selection_changed) =
                    handle_selection_pointer(&mut self.selection, frame_event);
                if selection_changed {
                    self.render()?;
                }
                if selection_consumed {
                    return Ok(());
                }

                if let Some(hit) = self.last_hit_map.resolve(frame_event.row, frame_event.col) {
                    let mut local_event = frame_event;
                    local_event.row = hit.local_row;
                    local_event.col = hit.local_col;
                    local_event.semantic = hit.local_semantic;
                    self.process_intent(Intent::PointerOn {
                        target: hit.node_id.to_string().into(),
                        event: local_event,
                    })
                } else {
                    self.process_intent(Intent::Pointer(frame_event))
                }
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
        match &intent {
            Intent::Exit => {
                // Compatibility fallback:
                // some terminals may collapse Ctrl+Shift+C to Ctrl+C.
                // If there is an active selection, prefer copy over exit.
                if self.selection.range().is_some() {
                    if let Err(err) = self.copy_selection_to_clipboard() {
                        eprintln!("failed to copy selection: {err}");
                    }
                    return Ok(());
                }
            }
            Intent::ScrollUp => {
                self.terminal.scroll(-1);
                return self.render();
            }
            Intent::ScrollDown => {
                self.terminal.scroll(1);
                return self.render();
            }
            Intent::ScrollPageUp => {
                let h = self.terminal.size().height as i32;
                self.terminal.scroll(-(h.saturating_sub(1)));
                return self.render();
            }
            Intent::ScrollPageDown => {
                let h = self.terminal.size().height as i32;
                self.terminal.scroll(h.saturating_sub(1));
                return self.render();
            }
            Intent::CopySelection => {
                if let Err(err) = self.copy_selection_to_clipboard() {
                    eprintln!("failed to copy selection: {err}");
                }
                return Ok(());
            }

            Intent::Submit
            | Intent::InputKey(_)
            | Intent::TextAction(_)
            | Intent::ToggleCompletion
            | Intent::CompleteNext
            | Intent::CompletePrev
            | Intent::NextFocus
            | Intent::PrevFocus
            | Intent::Cancel
            | Intent::Back
            | Intent::OpenOverlay(_)
            | Intent::OpenOverlayAtIndex(_)
            | Intent::OpenOverlayShortcut
            | Intent::CloseOverlay
            | Intent::Pointer(_)
            | Intent::PointerOn { .. } => {
                self.terminal.reset_scroll();
            }
            _ => {}
        }
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
        match action {
            WidgetAction::OpenUrl { url } => {
                if let Err(err) = clipboard::open_external_url(url.as_str()) {
                    eprintln!("failed to open URL '{}': {err}", url);
                }
                false
            }
            action => {
                let result = self.state.handle_action(action);
                self.finish_state_interaction(result)
            }
        }
    }

    fn apply_system_event(&mut self, event: SystemEvent) -> bool {
        let result = self.state.handle_system_event(event);
        self.finish_state_interaction(result)
    }

    fn finish_state_interaction(
        &mut self,
        result: steply_core::widgets::traits::InteractionResult,
    ) -> bool {
        self.flush_pending_scheduler_commands();
        self.flush_pending_task_invocations();
        result.request_render
    }

    fn flush_pending_scheduler_commands(&mut self) {
        for cmd in self.state.take_pending_scheduler_commands() {
            self.scheduler.schedule(cmd, Instant::now());
        }
    }

    fn flush_pending_task_invocations(&mut self) {
        for invocation in self.state.take_pending_task_invocations() {
            self.task_executor.spawn(invocation);
        }
    }

    fn render(&mut self) -> io::Result<()> {
        let view = RenderView::from_state(&self.state);
        let mut frame = self.renderer.render(&view, self.terminal.size());
        self.last_frame_lines = frame.lines.clone();
        if let Some(range) = self.selection.range() {
            apply_selection_highlight(&self.last_hit_map, &mut frame.lines, range);
        }
        self.last_hit_map = frame.hit_map.clone();
        self.terminal.render_frame(&frame)
    }

    fn selected_text(&self) -> Option<String> {
        let range = self.selection.range()?;
        extract_selected_text(&self.last_hit_map, &self.last_frame_lines, range)
    }

    fn copy_selection_to_clipboard(&self) -> io::Result<()> {
        let Some(text) = self.selected_text() else {
            return Ok(());
        };
        if text.is_empty() {
            return Ok(());
        }
        clipboard::copy_text_to_clipboard(text.as_str())
    }
}
