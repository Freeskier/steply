use crate::core::value::Value;
use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::task::{TaskId, TaskRequest};
use crate::ui::span::Span;
use crate::ui::spinner::SpinnerStyle;
use crate::ui::style::{Color, Style};
use crate::widgets::shared::task_watcher::{TaskWatcherState, TaskWatcherStatus};
use crate::widgets::traits::{DrawOutput, Drawable, InteractionResult, OutputNode, RenderContext};
use std::time::Instant;

pub struct TaskLogStep {
    pub label: String,
    pub task_id: TaskId,
}

impl TaskLogStep {
    pub fn new(label: impl Into<String>, task_id: impl Into<TaskId>) -> Self {
        Self {
            label: label.into(),
            task_id: task_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepStatus {
    Pending,
    Running,
    Done,
    Error,
}

struct StepState {
    label: String,
    task_id: TaskId,
    status: StepStatus,
    started_at: Option<Instant>,
    elapsed_secs: Option<f64>,
}

pub struct TaskLog {
    id: String,
    steps: Vec<StepState>,
    active: usize,
    watcher: TaskWatcherState,
}

impl TaskLog {
    pub fn new(id: impl Into<String>, steps: Vec<TaskLogStep>) -> Self {
        let steps = steps
            .into_iter()
            .map(|s| StepState {
                label: s.label,
                task_id: s.task_id,
                status: StepStatus::Pending,
                started_at: None,
                elapsed_secs: None,
            })
            .collect();
        Self {
            id: id.into(),
            steps,
            active: 0,
            watcher: TaskWatcherState::new(5, SpinnerStyle::Braille),
        }
    }

    pub fn watching(id: impl Into<String>, task_id: impl Into<TaskId>) -> Self {
        Self::new(id, vec![TaskLogStep::new("", task_id)])
    }

    pub fn with_visible_lines(mut self, n: usize) -> Self {
        self.watcher.set_visible_lines(n);
        self
    }

    pub fn with_spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.watcher.set_spinner_style(style);
        self
    }

    pub fn status(&self) -> TaskWatcherStatus {
        self.watcher.status()
    }

    pub fn push_step(&mut self, step: TaskLogStep) {
        self.steps.push(StepState {
            label: step.label,
            task_id: step.task_id,
            status: StepStatus::Pending,
            started_at: None,
            elapsed_secs: None,
        });
    }

    fn active_step(&self) -> Option<&StepState> {
        self.steps.get(self.active)
    }

    fn active_step_mut(&mut self) -> Option<&mut StepState> {
        self.steps.get_mut(self.active)
    }

    fn advance(&mut self, succeeded: bool) -> Option<TaskRequest> {
        if let Some(step) = self.active_step_mut() {
            step.elapsed_secs = step.started_at.map(|t| t.elapsed().as_secs_f64());
            step.status = if succeeded {
                StepStatus::Done
            } else {
                StepStatus::Error
            };
        }

        if !succeeded {
            return None;
        }

        let next = self.active + 1;
        if next >= self.steps.len() {
            return None;
        }

        self.active = next;
        if let Some(step) = self.active_step_mut() {
            step.status = StepStatus::Pending;
            step.started_at = None;
            step.elapsed_secs = None;
            let task_id = step.task_id.clone();
            self.watcher.request_start();
            Some(TaskRequest::new(task_id))
        } else {
            None
        }
    }

    fn mark_started(&mut self, run_id: u64) {
        if let Some(step) = self.active_step_mut() {
            step.status = StepStatus::Running;
            if step.started_at.is_none() {
                step.started_at = Some(Instant::now());
            }
        }
        self.watcher.mark_started(run_id);
    }

    fn mark_start_rejected(&mut self, reason: &str) {
        if let Some(step) = self.active_step_mut() {
            step.status = StepStatus::Error;
            step.elapsed_secs = step.started_at.map(|t| t.elapsed().as_secs_f64());
        }
        self.watcher.mark_rejected(reason.to_string());
    }

    fn render_step_line(&self, step: &StepState, index: usize, total: usize) -> Vec<Span> {
        let counter = format!("[{}/{}]", index + 1, total);
        let show_counter = total > 1;
        let dim = Style::new().color(Color::DarkGrey);
        let normal = Style::new().color(Color::White);

        match step.status {
            StepStatus::Pending => {
                let mut line = Vec::new();
                if show_counter {
                    line.push(Span::styled(counter, dim).no_wrap());
                    line.push(Span::new(" ").no_wrap());
                }
                line.push(Span::styled(step.label.clone(), normal).no_wrap());
                line
            }
            StepStatus::Running => {
                let elapsed = step
                    .started_at
                    .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                    .unwrap_or_default();
                let mut line = Vec::new();
                if show_counter {
                    line.push(Span::styled(counter, dim).no_wrap());
                    line.push(Span::new(" ").no_wrap());
                }
                line.push(self.watcher.spinner().span());
                line.push(Span::new(" ").no_wrap());
                line.push(
                    Span::styled(
                        format!("{}...", step.label),
                        Style::new().color(Color::White).bold(),
                    )
                    .no_wrap(),
                );
                line.push(Span::styled(elapsed, dim).no_wrap());
                line
            }
            StepStatus::Done => {
                let elapsed = step
                    .elapsed_secs
                    .map(|s| format!("  {:.1}s", s))
                    .unwrap_or_default();
                let mut line = Vec::new();
                if show_counter {
                    line.push(Span::styled(counter, dim).no_wrap());
                    line.push(Span::new(" ").no_wrap());
                }
                line.push(Span::styled("✓", Style::new().color(Color::Green).bold()).no_wrap());
                line.push(Span::new(" ").no_wrap());
                line.push(Span::styled(step.label.clone(), normal).no_wrap());
                line.push(Span::styled(elapsed, dim).no_wrap());
                line
            }
            StepStatus::Error => {
                let elapsed = step
                    .elapsed_secs
                    .map(|s| format!("  {:.1}s", s))
                    .unwrap_or_default();
                let mut line = Vec::new();
                if show_counter {
                    line.push(Span::styled(counter, dim).no_wrap());
                    line.push(Span::new(" ").no_wrap());
                }
                line.push(Span::styled("✗", Style::new().color(Color::Red).bold()).no_wrap());
                line.push(Span::new(" ").no_wrap());
                line.push(Span::styled(step.label.clone(), normal).no_wrap());
                line.push(Span::styled(elapsed, dim).no_wrap());
                line
            }
        }
    }
}

impl Drawable for TaskLog {
    fn id(&self) -> &str {
        &self.id
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let total = self.steps.len();
        let single_mode = total == 1 && self.steps[0].label.is_empty();
        let mut lines = Vec::new();

        if !single_mode {
            for (i, step) in self.steps.iter().enumerate() {
                if step.status == StepStatus::Pending && i != self.active {
                    continue;
                }
                lines.push(self.render_step_line(step, i, total));
            }
        } else if let Some(step) = self.steps.first() {
            match step.status {
                StepStatus::Pending => {
                    if self.watcher.status() == TaskWatcherStatus::Pending {
                        lines.push(vec![
                            Span::styled("…", Style::new().color(Color::Blue).bold()).no_wrap(),
                            Span::new(" Starting...").no_wrap(),
                        ]);
                    }
                }
                StepStatus::Running => {}
                StepStatus::Done => lines.push(vec![
                    Span::styled("✓", Style::new().color(Color::Green).bold()).no_wrap(),
                    Span::new(" Done").no_wrap(),
                ]),
                StepStatus::Error => lines.push(vec![
                    Span::styled("✗", Style::new().color(Color::Red).bold()).no_wrap(),
                    Span::new(" Failed").no_wrap(),
                ]),
            }
        }

        let show_logs = self
            .active_step()
            .is_some_and(|s| s.status == StepStatus::Running || s.status == StepStatus::Error);
        if show_logs {
            for line in self.watcher.logs() {
                lines.push(vec![
                    Span::styled(format!("  {line}"), Style::new().color(Color::DarkGrey))
                        .no_wrap(),
                ]);
            }
        }

        DrawOutput { lines }
    }
}

impl OutputNode for TaskLog {
    fn on_tick(&mut self) -> InteractionResult {
        if self.watcher.tick() {
            return InteractionResult::handled();
        }
        InteractionResult::ignored()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        match event {
            SystemEvent::TaskStarted { task_id, run_id } => {
                let is_active = self.active_step().is_some_and(|s| &s.task_id == task_id);
                if !is_active {
                    return InteractionResult::ignored();
                }
                self.mark_started(*run_id);
                InteractionResult::handled()
            }
            SystemEvent::TaskStartRejected { task_id, reason } => {
                let is_active = self.active_step().is_some_and(|s| &s.task_id == task_id);
                if !is_active {
                    return InteractionResult::ignored();
                }
                self.mark_start_rejected(reason.as_str());
                InteractionResult::handled()
            }
            SystemEvent::TaskLogLine {
                task_id,
                run_id,
                line,
            } => {
                let is_active = self.active_step().is_some_and(|s| &s.task_id == task_id);
                if !is_active {
                    return InteractionResult::ignored();
                }
                if self.watcher.append_log(*run_id, line.clone()) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            SystemEvent::TaskCompleted { completion } => {
                let is_active = self
                    .active_step()
                    .is_some_and(|s| s.task_id == completion.task_id);
                if !is_active {
                    return InteractionResult::ignored();
                }

                let succeeded = completion.error.is_none() && !completion.cancelled;
                if !self.watcher.mark_completed(completion.run_id, succeeded) {
                    return InteractionResult::ignored();
                }

                if let Some(request) = self.advance(succeeded) {
                    return InteractionResult::with_action(WidgetAction::TaskRequested { request });
                }
                if succeeded {
                    return InteractionResult::input_done();
                }
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        None
    }
}

impl TaskLog {
    pub fn start_request(&mut self) -> Option<TaskRequest> {
        if self.steps.is_empty() {
            return None;
        }
        self.active = 0;
        for step in &mut self.steps {
            step.status = StepStatus::Pending;
            step.started_at = None;
            step.elapsed_secs = None;
        }
        self.watcher.request_start();
        Some(TaskRequest::new(self.steps[0].task_id.clone()))
    }
}
