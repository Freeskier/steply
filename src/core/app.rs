use crate::action_bindings::ActionBindings;
use crate::array_input::ArrayInput;
use crate::checkbox_input::CheckboxInput;
use crate::choice_input::ChoiceInput;
use crate::color_input::ColorInput;
use crate::event::Action;
use crate::event_queue::{AppEvent, EventQueue};
use crate::flow::Flow;
use crate::frame::Line;
use crate::layout::Layout;
use crate::node::{Node, RenderMode};
use crate::overlay::OverlayState;
use crate::password_input::{PasswordInput, PasswordRender};
use crate::path_input::PathInput;
use crate::reducer::{Effect, Reducer};
use crate::renderer::Renderer;
use crate::segmented_input::SegmentedInput;
use crate::select_input::SelectInput;
use crate::slider_input::SliderInput;
use crate::span::{Span, Wrap};
use crate::state::AppState;
use crate::step::Step;
use crate::terminal::KeyEvent;
use crate::terminal::Terminal;
use crate::text_input::TextInput;
use crate::theme::Theme;
use crate::validators;
use crate::view_state::ErrorDisplay;
use std::io;
use std::time::{Duration, Instant};

const ERROR_TIMEOUT: Duration = Duration::from_secs(2);
const ENABLE_DECORATION: bool = true;
const APP_TITLE: &str = "Steply main";
const TAB_ADVANCE_DELAY: Duration = Duration::from_millis(300);

pub struct App {
    pub state: AppState,
    pub renderer: Renderer,
    action_bindings: ActionBindings,
    event_queue: EventQueue,
    theme: Theme,
    last_rendered_step: usize,
    pending_tab_advance_at: Option<Instant>,
    pending_tab_input_id: Option<String>,
    overlay_render_start: Option<u16>,
    overlay_render_lines: usize,
    overlay_focus_id: Option<String>,
    overlay: Option<OverlayState>,
    last_step_cursor: Option<(u16, u16)>,
}

impl App {
    pub fn new() -> Self {
        let flow = Flow::new(build_steps());
        let mut app = Self {
            state: AppState::new(flow),
            renderer: Renderer::new(),
            action_bindings: ActionBindings::new(),
            event_queue: EventQueue::new(),
            theme: Theme::default_theme(),
            last_rendered_step: 0,
            pending_tab_advance_at: None,
            pending_tab_input_id: None,
            overlay_render_start: None,
            overlay_render_lines: 0,
            overlay_focus_id: None,
            overlay: None,
            last_step_cursor: None,
        };

        app.renderer.set_decoration_enabled(ENABLE_DECORATION);
        app.renderer.set_title(APP_TITLE);
        app
    }

    pub fn tick(&mut self) -> bool {
        let mut processed_any = false;
        self.maybe_fire_pending_tab_advance();
        loop {
            let now = Instant::now();
            let Some(event) = self.event_queue.next_ready(now) else {
                break;
            };
            self.dispatch_event(event);
            processed_any = true;
        }
        processed_any
    }

    pub fn render(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        self.renderer.render_title_once(terminal, &self.theme)?;
        terminal.queue_hide_cursor()?;

        let current_step = self.state.flow.current_index();
        if current_step != self.last_rendered_step {
            if let Some(step) = self.state.flow.step_at(self.last_rendered_step) {
                let done_view = crate::view_state::ViewState::new();
                self.renderer.render_with_status_without_cursor(
                    step,
                    &done_view,
                    &self.theme,
                    terminal,
                    crate::flow::StepStatus::Done,
                    true,
                )?;
            }
            self.renderer.move_to_end(terminal)?;
            self.renderer.write_connector_lines(
                terminal,
                &self.theme,
                crate::flow::StepStatus::Done,
                1,
            )?;
            self.renderer.reset_block();
            self.last_rendered_step = current_step;
        }

        self.clear_overlay_region(terminal)?;
        let step_cursor = self.renderer.render_with_status_plan(
            self.state.flow.current_step(),
            &self.state.view,
            &self.theme,
            terminal,
            self.state.flow.current_status(),
            false,
        )?;
        if step_cursor.is_some() {
            self.last_step_cursor = step_cursor;
        }

        let overlay_cursor = self.render_overlay(terminal, step_cursor)?;
        let final_cursor = overlay_cursor.or(step_cursor);
        if let Some((col, row)) = final_cursor {
            terminal.queue_move_cursor(col, row)?;
        }
        terminal.queue_show_cursor()?;
        terminal.flush()?;

        Ok(())
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) {
        self.event_queue.emit(AppEvent::Key(key_event));
    }

    pub fn should_exit(&self) -> bool {
        self.state.should_exit
    }

    fn dispatch_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key_event) => {
                if self.overlay.is_some() {
                    if matches!(
                        key_event.code,
                        crate::terminal::KeyCode::Esc | crate::terminal::KeyCode::Enter
                    ) {
                        self.close_overlay();
                        return;
                    }
                }

                if key_event.code == crate::terminal::KeyCode::Char('o')
                    && key_event.modifiers == crate::terminal::KeyModifiers::CONTROL
                    && self.overlay.is_none()
                {
                    self.open_overlay();
                    return;
                }

                if !matches!(key_event.code, crate::terminal::KeyCode::Tab)
                    || key_event.modifiers != crate::terminal::KeyModifiers::NONE
                {
                    self.clear_pending_tab();
                }

                if matches!(key_event.code, crate::terminal::KeyCode::Tab)
                    && key_event.modifiers == crate::terminal::KeyModifiers::NONE
                    && self.handle_tab_completion(key_event)
                {
                    return;
                }

                let captured = self
                    .state
                    .engine
                    .focused_input_caps(self.state.flow.current_step())
                    .map(|caps| caps.captures_key(key_event.code, key_event.modifiers))
                    .unwrap_or(false);

                if !captured {
                    if let Some(action) = self.action_bindings.handle_key(&key_event) {
                        if self.overlay.is_some()
                            && matches!(action, Action::NextInput | Action::PrevInput)
                        {
                            let direction = match action {
                                Action::NextInput => 1,
                                Action::PrevInput => -1,
                                _ => 0,
                            };
                            self.move_overlay_focus(direction);
                            return;
                        }
                        let effects = Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT);
                        self.apply_effects(effects);
                        return;
                    }
                }

                let effects =
                    Reducer::reduce(&mut self.state, Action::InputKey(key_event), ERROR_TIMEOUT);
                self.apply_effects(effects);
            }
            AppEvent::Action(action) => {
                if self.overlay.is_some() && matches!(action, Action::NextInput | Action::PrevInput)
                {
                    let direction = match action {
                        Action::NextInput => 1,
                        Action::PrevInput => -1,
                        _ => 0,
                    };
                    self.move_overlay_focus(direction);
                    return;
                }
                let effects = Reducer::reduce(&mut self.state, action, ERROR_TIMEOUT);
                self.apply_effects(effects);
            }
            AppEvent::RequestRerender => {}
            AppEvent::InputChanged { .. } | AppEvent::FocusChanged { .. } | AppEvent::Submitted => {
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Emit(event) => self.event_queue.emit(event),
                Effect::EmitAfter(event, delay) => self.event_queue.emit_after(event, delay),
                Effect::CancelClearError(id) => self.event_queue.cancel_clear_error_message(&id),
            }
        }
    }

    fn handle_tab_completion(&mut self, key_event: crate::terminal::KeyEvent) -> bool {
        let step = self.state.flow.current_step();
        let Some(input) = self.state.engine.focused_input(step) else {
            return false;
        };
        if !input.supports_tab_completion() {
            return false;
        }

        let now = Instant::now();
        let focused_id = self.state.engine.focused_input_id(step);

        if let Some(due) = self.pending_tab_advance_at {
            if now <= due && focused_id == self.pending_tab_input_id {
                self.clear_pending_tab();
                let effects =
                    Reducer::reduce(&mut self.state, Action::InputKey(key_event), ERROR_TIMEOUT);
                self.apply_effects(effects);
                return true;
            }
        }

        self.pending_tab_advance_at = Some(now + TAB_ADVANCE_DELAY);
        self.pending_tab_input_id = focused_id;
        true
    }

    fn maybe_fire_pending_tab_advance(&mut self) {
        let Some(due) = self.pending_tab_advance_at else {
            return;
        };
        if Instant::now() < due {
            return;
        }

        let step = self.state.flow.current_step();
        let focused_id = self.state.engine.focused_input_id(step);
        if focused_id != self.pending_tab_input_id {
            self.clear_pending_tab();
            return;
        }

        self.clear_pending_tab();
        let effects = Reducer::reduce(&mut self.state, Action::NextInput, ERROR_TIMEOUT);
        self.apply_effects(effects);
    }

    fn clear_pending_tab(&mut self) {
        self.pending_tab_advance_at = None;
        self.pending_tab_input_id = None;
    }

    pub fn request_rerender(&mut self) {
        self.event_queue.emit(AppEvent::RequestRerender);
    }

    fn open_overlay(&mut self) {
        let (overlay, nodes) = OverlayState::demo();
        let step = self.state.flow.current_step_mut();
        self.overlay_focus_id = self.state.engine.focused_input_id(step);
        step.nodes.extend(nodes);
        self.state.engine.reset(step);

        if let Some(first_id) = overlay.input_ids.first() {
            if let Some(pos) = self.state.engine.find_input_pos_by_id(step, first_id) {
                let mut events = Vec::new();
                self.state.engine.update_focus(step, Some(pos), &mut events);
                for event in events {
                    self.event_queue.emit(event);
                }
            }
        }

        self.overlay = Some(overlay);
        self.event_queue.emit(AppEvent::RequestRerender);
    }

    fn close_overlay(&mut self) {
        if self.overlay.take().is_none() {
            return;
        }
        let step = self.state.flow.current_step_mut();
        step.nodes.retain(|node| !node.is_overlay());
        self.state.engine.reset(step);
        self.restore_overlay_focus();
        self.event_queue.emit(AppEvent::RequestRerender);
    }

    fn move_overlay_focus(&mut self, direction: isize) {
        let Some(overlay) = self.overlay.as_ref() else {
            return;
        };
        let step = self.state.flow.current_step();
        let positions = self.overlay_input_positions(step, overlay);
        if positions.is_empty() {
            return;
        }
        let current_pos = self.state.engine.focused_index();
        let current_idx = current_pos
            .and_then(|pos| positions.iter().position(|candidate| *candidate == pos))
            .unwrap_or(0);
        let len = positions.len() as isize;
        let next_idx = (current_idx as isize + direction + len) % len;
        let next_pos = positions[next_idx as usize];
        let step = self.state.flow.current_step_mut();
        let mut events = Vec::new();
        self.state
            .engine
            .update_focus(step, Some(next_pos), &mut events);
        for event in events {
            self.event_queue.emit(event);
        }
    }

    fn restore_overlay_focus(&mut self) {
        let Some(id) = self.overlay_focus_id.take() else {
            return;
        };
        let step = self.state.flow.current_step_mut();
        if let Some(pos) = self.state.engine.find_input_pos_by_id(step, &id) {
            let mut events = Vec::new();
            self.state.engine.update_focus(step, Some(pos), &mut events);
            for event in events {
                self.event_queue.emit(event);
            }
        }
    }

    fn overlay_input_positions(&self, step: &Step, overlay: &OverlayState) -> Vec<usize> {
        overlay
            .input_ids
            .iter()
            .filter_map(|id| self.state.engine.find_input_pos_by_id(step, id))
            .collect()
    }

    fn build_overlay_render_lines(
        &self,
        step: &Step,
        overlay: &OverlayState,
    ) -> Vec<(Vec<Span>, Option<usize>)> {
        let mut lines = Vec::new();

        if !overlay.label.is_empty() {
            lines.push((
                vec![Span::new(overlay.label.clone()).with_style(self.theme.prompt.clone())],
                None,
            ));
        }

        if let Some(hint) = overlay.hint.as_ref() {
            if !hint.is_empty() {
                lines.push((
                    vec![Span::new(hint.clone()).with_style(self.theme.hint.clone())],
                    None,
                ));
            }
        }

        for node in self.overlay_nodes(step, overlay) {
            let inline_error = match node.as_input() {
                Some(input) => matches!(
                    self.state.view.error_display(input.id()),
                    ErrorDisplay::InlineMessage
                ),
                None => false,
            };
            let spans = node.render(RenderMode::Full, inline_error, &self.theme);
            let cursor_offset = node.cursor_offset();
            lines.push((spans, cursor_offset));
        }

        lines
    }

    fn overlay_nodes<'a>(&self, step: &'a Step, overlay: &OverlayState) -> Vec<&'a Node> {
        overlay
            .input_ids
            .iter()
            .filter_map(|id| {
                step.nodes.iter().find(|node| {
                    node.as_input()
                        .is_some_and(|input| input.id() == id.as_str())
                })
            })
            .collect()
    }

    fn overlay_separator_lines(&self, width: u16) -> (Line, Line) {
        let mut line = Line::new();
        let glyph = Span::new("›")
            .with_style(self.theme.decor_accent.clone())
            .with_wrap(Wrap::No);
        line.push(glyph);
        let dash_count = width.saturating_sub(1) as usize;
        if dash_count > 0 {
            line.push(
                Span::new("─".repeat(dash_count))
                    .with_style(self.theme.decor_done.clone())
                    .with_wrap(Wrap::No),
            );
        }
        (line.clone(), line)
    }

    fn render_overlay(
        &mut self,
        terminal: &mut Terminal,
        step_cursor: Option<(u16, u16)>,
    ) -> io::Result<Option<(u16, u16)>> {
        if let Some(overlay) = self.overlay.as_ref() {
            let step = self.state.flow.current_step();
            let anchor_cursor = step_cursor.or(self.last_step_cursor);
            let start = anchor_cursor.map(|(_, row)| row + 1).unwrap_or_else(|| {
                let _ = self.renderer.move_to_end(terminal);
                let _ = terminal.refresh_cursor_position();
                terminal.cursor_position().y
            });
            let width = terminal.size().width;
            let start_col = self.renderer.overlay_padding() as u16;
            let available = width.saturating_sub(start_col);
            let render_lines = self.build_overlay_render_lines(step, overlay);
            let (frame, cursor) =
                Layout::new().compose_spans_with_cursor(render_lines.into_iter(), available);
            let lines = frame.lines();
            let separators = self.overlay_separator_lines(width);
            let (count, cursor) = self.renderer.render_overlay(
                terminal,
                start,
                start_col,
                width,
                lines,
                self.overlay_render_lines,
                cursor,
                separators,
            )?;
            self.overlay_render_start = Some(start);
            self.overlay_render_lines = count;
            return Ok(cursor);
        }

        Ok(None)
    }

    fn clear_overlay_region(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        if self.overlay.is_some() {
            return Ok(());
        }
        let Some(start) = self.overlay_render_start.take() else {
            return Ok(());
        };
        if self.overlay_render_lines > 0 {
            for idx in 0..self.overlay_render_lines {
                let line_row = start + idx as u16;
                terminal.queue_move_cursor(0, line_row)?;
                terminal.queue_clear_line()?;
            }
            terminal.flush()?;
        }
        self.overlay_render_lines = 0;
        Ok(())
    }
}

fn build_step() -> Step {
    Step {
        prompt: "Please fill the form:".to_string(),
        hint: Some("Press Tab/Shift+Tab to navigate, Enter to submit, Esc to exit".to_string()),
        nodes: vec![
            Node::input(
                TextInput::new("username", "Username")
                    .with_validator(validators::required())
                    .with_validator(validators::min_length(3)),
            ),
            Node::input(
                TextInput::new("email", "Email")
                    .with_validator(validators::required())
                    .with_validator(validators::email()),
            ),
            Node::input(ColorInput::new("accent_color", "Accent Color").with_rgb(64, 120, 200)),
            Node::input(CheckboxInput::new("tos", "Accept Terms").with_checked(true)),
            Node::input(
                ChoiceInput::new(
                    "plan",
                    "Plan",
                    vec!["Free".to_string(), "Pro".to_string(), "Team".to_string()],
                )
                .with_bullets(true),
            ),
            Node::input(ArrayInput::new("tags", "Tags")),
            Node::input(PathInput::new("path", "Path")),
            Node::input(SelectInput::new(
                "color",
                "Color",
                vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            )),
        ],
        form_validators: Vec::new(),
    }
}

fn build_step_two() -> Step {
    Step {
        prompt: "Almost there:".to_string(),
        hint: None,
        nodes: vec![Node::input(
            TextInput::new("password", "Password")
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )],
        form_validators: Vec::new(),
    }
}

fn build_step_three() -> Step {
    Step {
        prompt: "Final step:".to_string(),
        hint: Some("Try arrows left/right in select, and masked input".to_string()),
        nodes: vec![
            Node::input(
                PasswordInput::new("new_password", "New Password")
                    .with_render_mode(PasswordRender::Stars)
                    .with_validator(validators::required())
                    .with_validator(validators::min_length(8)),
            ),
            Node::input(SliderInput::new("email", "Email", 1, 20)),
            Node::input(SegmentedInput::ipv4("ip_address", "IP Address")),
            Node::input(SegmentedInput::phone_us("phone", "Phone")),
            Node::input(SegmentedInput::number("num", "num")),
            Node::input(SegmentedInput::date_dd_mm_yyyy(
                "birthdate_masked",
                "Birth Date",
            )),
        ],
        form_validators: Vec::new(),
    }
}

fn build_steps() -> Vec<Step> {
    vec![build_step(), build_step_two(), build_step_three()]
}
