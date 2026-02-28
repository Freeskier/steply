use crate::runtime::effect::Effect;
use crate::runtime::event::{AppEvent, SystemEvent, WidgetAction};
use crate::runtime::intent::Intent;
use crate::runtime::key_bindings::KeyBindings;
use crate::runtime::reducer::Reducer;
use crate::runtime::scheduler::Scheduler;
use crate::state::app::AppState;
use crate::task::{LogLine, TaskExecutor};
use crate::terminal::{
    KeyModifiers, PointerButton, PointerEvent, PointerKind, RenderMode, Terminal, TerminalEvent,
};
use crate::ui::frame_json::frame_to_json;
use crate::ui::hit_test::FrameHitMap;
use crate::ui::render_view::RenderView;
use crate::ui::renderer::{Renderer, RendererConfig};
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::ui::text::{split_prefix_at_display_width, text_display_width};
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionPoint {
    row: u16,
    col: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionRange {
    start: SelectionPoint,
    end: SelectionPoint,
}

#[derive(Debug, Clone, Copy, Default)]
struct SelectionState {
    anchor: Option<SelectionPoint>,
    head: Option<SelectionPoint>,
    pending_anchor: Option<SelectionPoint>,
    dragging: bool,
}

impl SelectionState {
    fn begin(&mut self, at: SelectionPoint) -> bool {
        let changed = self.anchor != Some(at) || self.head != Some(at) || !self.dragging;
        self.anchor = Some(at);
        self.head = Some(at);
        self.pending_anchor = None;
        self.dragging = true;
        changed
    }

    fn update(&mut self, at: SelectionPoint) -> bool {
        if !self.dragging {
            return false;
        }
        let changed = self.head != Some(at);
        self.head = Some(at);
        changed
    }

    fn end(&mut self, at: SelectionPoint) -> bool {
        if !self.dragging {
            return false;
        }
        let changed = self.head != Some(at) || self.dragging;
        self.head = Some(at);
        self.dragging = false;
        self.pending_anchor = None;
        changed
    }

    fn set_pending_anchor(&mut self, at: SelectionPoint) -> bool {
        let had_selection = self.anchor.is_some() || self.head.is_some() || self.dragging;
        self.anchor = None;
        self.head = None;
        self.dragging = false;
        self.pending_anchor = Some(at);
        had_selection
    }

    fn begin_from_pending_or(&mut self, at: SelectionPoint) -> bool {
        let anchor = self.pending_anchor.unwrap_or(at);
        self.begin(anchor)
    }

    fn range(&self) -> Option<SelectionRange> {
        let (Some(anchor), Some(head)) = (self.anchor, self.head) else {
            return None;
        };
        if anchor == head {
            return None;
        }
        let (start, end) = if (anchor.row, anchor.col) <= (head.row, head.col) {
            (anchor, head)
        } else {
            (head, anchor)
        };
        Some(SelectionRange { start, end })
    }
}

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

    pub fn print_render_json(&mut self) -> io::Result<()> {
        let view = RenderView::from_state(&self.state);
        let size = self.terminal.size();
        let frame = self.renderer.render(&view, size);
        let doc = frame_to_json(&frame, size);

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
                    self.handle_selection_pointer(frame_event);
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
                if let Err(err) = Self::open_external_url(url.as_str()) {
                    eprintln!("failed to open URL '{}': {err}", url);
                }
                false
            }
            action => {
                let result = self.state.handle_action(action);
                self.flush_pending_scheduler_commands();
                self.flush_pending_task_invocations();
                result.request_render
            }
        }
    }

    fn apply_system_event(&mut self, event: SystemEvent) -> bool {
        let result = self.state.handle_system_event(event);
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
            self.apply_selection_highlight(&mut frame.lines, range);
        }
        self.last_hit_map = frame.hit_map.clone();
        self.terminal.render_frame(&frame)
    }

    fn handle_selection_pointer(&mut self, event: PointerEvent) -> (bool, bool) {
        let point = SelectionPoint {
            row: event.row,
            col: event.col,
        };
        match event.kind {
            PointerKind::Down(PointerButton::Left) => {
                if event.modifiers.contains(KeyModifiers::SHIFT) {
                    return (true, self.selection.begin(point));
                }
                let changed = self.selection.set_pending_anchor(point);
                (false, changed)
            }
            PointerKind::Drag(PointerButton::Left) => {
                if self.selection.dragging {
                    return (true, self.selection.update(point));
                }
                if self.selection.pending_anchor.is_some()
                    || event.modifiers.contains(KeyModifiers::SHIFT)
                {
                    let mut changed = self.selection.begin_from_pending_or(point);
                    changed |= self.selection.update(point);
                    return (true, changed);
                }
                (false, false)
            }
            PointerKind::Up(PointerButton::Left) => {
                if self.selection.dragging {
                    return (true, self.selection.end(point));
                }
                let changed = self.selection.pending_anchor.take().is_some();
                (false, changed)
            }
            _ => (false, false),
        }
    }

    fn apply_selection_highlight(&self, lines: &mut [SpanLine], range: SelectionRange) {
        if lines.is_empty() {
            return;
        }

        let mut start_row = range.start.row as usize;
        let mut end_row = range.end.row as usize;
        if start_row >= lines.len() {
            return;
        }
        if end_row >= lines.len() {
            end_row = lines.len().saturating_sub(1);
        }
        if start_row > end_row {
            std::mem::swap(&mut start_row, &mut end_row);
        }

        for (row_idx, line) in lines
            .iter_mut()
            .enumerate()
            .take(end_row + 1)
            .skip(start_row)
        {
            let line_width = display_width_for_line(line.as_slice()) as u16;
            let row_start = if row_idx == start_row {
                range.start.col.min(line_width)
            } else {
                0
            };
            let row_end = if row_idx == end_row {
                range.end.col.min(line_width)
            } else {
                line_width
            };
            if row_end <= row_start {
                continue;
            }
            let selectable = selectable_ranges_for_row(
                &self.last_hit_map,
                row_idx as u16,
                line_width,
            );
            if selectable.is_empty() {
                continue;
            }
            for (sel_start, sel_end) in selectable {
                let start = row_start.max(sel_start).min(line_width);
                let end = row_end.min(sel_end).min(line_width);
                if end > start {
                    highlight_line_range(line, start as usize, end as usize);
                }
            }
        }
    }

    fn selected_text(&self) -> Option<String> {
        let range = self.selection.range()?;
        if self.last_frame_lines.is_empty() {
            return None;
        }
        let start_row = range.start.row as usize;
        if start_row >= self.last_frame_lines.len() {
            return None;
        }
        let end_row = (range.end.row as usize).min(self.last_frame_lines.len() - 1);
        if end_row < start_row {
            return None;
        }

        let mut rows = Vec::<String>::new();
        for row_idx in start_row..=end_row {
            let line = &self.last_frame_lines[row_idx];
            let line_width = display_width_for_line(line.as_slice()) as u16;
            let row_start = if row_idx == start_row {
                range.start.col.min(line_width)
            } else {
                0
            };
            let row_end = if row_idx == end_row {
                range.end.col.min(line_width)
            } else {
                line_width
            };
            if row_end <= row_start {
                continue;
            }

            let selectable = selectable_ranges_for_row(
                &self.last_hit_map,
                row_idx as u16,
                line_width,
            );
            if selectable.is_empty() {
                continue;
            }
            let mut row_text = String::new();
            for (sel_start, sel_end) in selectable {
                let start = row_start.max(sel_start).min(line_width);
                let end = row_end.min(sel_end).min(line_width);
                if end > start {
                    row_text.push_str(&extract_line_text_range(
                        line.as_slice(),
                        start as usize,
                        end as usize,
                    ));
                }
            }
            if !row_text.is_empty() {
                rows.push(row_text);
            }
        }
        if rows.is_empty() {
            None
        } else {
            Some(rows.join("\n"))
        }
    }

    fn copy_selection_to_clipboard(&self) -> io::Result<()> {
        let Some(text) = self.selected_text() else {
            return Ok(());
        };
        if text.is_empty() {
            return Ok(());
        }
        Self::copy_text_to_clipboard(text.as_str())
    }

    fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        {
            return run_clipboard_command("cmd", &["/C", "clip"], text)
                .or_else(|_| copy_via_osc52(text));
        }

        #[cfg(target_os = "macos")]
        {
            return run_clipboard_command("pbcopy", &[], text).or_else(|_| copy_via_osc52(text));
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let candidates: [(&str, &[&str]); 3] = [
                ("wl-copy", &[]),
                ("xclip", &["-selection", "clipboard"]),
                ("xsel", &["--clipboard", "--input"]),
            ];
            let mut last_error: Option<io::Error> = None;
            for (program, args) in candidates {
                match run_clipboard_command(program, args, text) {
                    Ok(()) => return Ok(()),
                    Err(err) => last_error = Some(err),
                }
            }
            return copy_via_osc52(text).map_err(|_| {
                last_error.unwrap_or_else(|| io::Error::other("clipboard unavailable"))
            });
        }

        #[allow(unreachable_code)]
        Err(io::Error::other("unsupported platform for clipboard"))
    }

    fn open_external_url(url: &str) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", ""])
                .arg(url)
                .spawn()?;
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).spawn()?;
            return Ok(());
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open").arg(url).spawn()?;
            return Ok(());
        }

        #[allow(unreachable_code)]
        Err(io::Error::other("unsupported platform for URL opening"))
    }
}

fn selection_highlight_style() -> Style {
    Style::new().background(Color::Blue)
}

fn display_width_for_line(line: &[Span]) -> usize {
    line.iter()
        .map(|span| text_display_width(span.text.as_str()))
        .sum()
}

fn selectable_ranges_for_row(
    hit_map: &FrameHitMap,
    row: u16,
    line_width: u16,
) -> Vec<(u16, u16)> {
    let from_hit_map = hit_map.row_ranges(row);
    if !from_hit_map.is_empty() {
        return from_hit_map;
    }
    fallback_selectable_ranges(line_width)
}

fn fallback_selectable_ranges(line_width: u16) -> Vec<(u16, u16)> {
    if line_width == 0 {
        return Vec::new();
    }
    vec![(0, line_width)]
}

fn highlight_line_range(line: &mut SpanLine, start_col: usize, end_col: usize) {
    if end_col <= start_col || line.is_empty() {
        return;
    }

    let mut out = Vec::<Span>::with_capacity(line.len().saturating_mul(2));
    let mut col = 0usize;
    for span in line.iter() {
        let width = text_display_width(span.text.as_str());
        if width == 0 {
            out.push(span.clone());
            continue;
        }

        let span_start = col;
        let span_end = col.saturating_add(width);
        let sel_start = start_col.max(span_start);
        let sel_end = end_col.min(span_end);
        if sel_end <= sel_start {
            out.push(span.clone());
            col = span_end;
            continue;
        }

        let left_width = sel_start.saturating_sub(span_start);
        let mid_width = sel_end.saturating_sub(sel_start);
        let (left, tail) = split_prefix_at_display_width(span.text.as_str(), left_width);
        let (mid, right) = split_prefix_at_display_width(tail, mid_width);

        if !left.is_empty() {
            let mut piece = span.clone();
            piece.text = left.to_string();
            out.push(piece);
        }
        if !mid.is_empty() {
            let mut piece = span.clone();
            piece.text = mid.to_string();
            piece.style = span.style.merge(selection_highlight_style());
            out.push(piece);
        }
        if !right.is_empty() {
            let mut piece = span.clone();
            piece.text = right.to_string();
            out.push(piece);
        }

        col = span_end;
    }

    *line = out;
}

fn extract_line_text_range(line: &[Span], start_col: usize, end_col: usize) -> String {
    if end_col <= start_col || line.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut col = 0usize;
    for span in line {
        let width = text_display_width(span.text.as_str());
        if width == 0 {
            continue;
        }

        let span_start = col;
        let span_end = col.saturating_add(width);
        let sel_start = start_col.max(span_start);
        let sel_end = end_col.min(span_end);
        if sel_end <= sel_start {
            col = span_end;
            continue;
        }

        let left_width = sel_start.saturating_sub(span_start);
        let mid_width = sel_end.saturating_sub(sel_start);
        let (_, tail) = split_prefix_at_display_width(span.text.as_str(), left_width);
        let (mid, _) = split_prefix_at_display_width(tail, mid_width);
        out.push_str(mid);

        col = span_end;
    }

    out
}

fn run_clipboard_command(program: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{program} exited with status {status}"
        )))
    }
}

fn copy_via_osc52(text: &str) -> io::Result<()> {
    // OSC 52: set clipboard from terminal host (works in modern terminal emulators/SSH chains).
    let payload = base64_encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{payload}\x07");
    let mut out = io::stdout();
    out.write_all(sequence.as_bytes())?;
    out.flush()?;
    Ok(())
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if input.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0usize;
    while i + 3 <= input.len() {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let b2 = input[i + 2] as u32;
        i += 3;

        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }

    let rem = input.len() - i;
    if rem == 1 {
        let b0 = input[i] as u32;
        let n = b0 << 16;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let n = (b0 << 16) | (b1 << 8);
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }

    out
}
