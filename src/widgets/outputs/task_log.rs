use crate::core::value::Value;
use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::task::{TaskId, TaskRequest};
use crate::ui::span::Span;
use crate::ui::spinner::{Spinner, SpinnerStyle};
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, InteractionResult, OutputNode, RenderContext};
use std::collections::VecDeque;
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
    logs: VecDeque<String>,
    visible_lines: usize,
    spinner: Spinner,
}

impl TaskLog {

    pub fn new(id: impl Into<String>, steps: Vec<TaskLogStep>) -> Self {
        let steps = steps
            .into_iter()
            .enumerate()
            .map(|(i, s)| StepState {
                label: s.label,
                task_id: s.task_id,

                status: if i == 0 {
                    StepStatus::Running
                } else {
                    StepStatus::Pending
                },
                started_at: if i == 0 { Some(Instant::now()) } else { None },
                elapsed_secs: None,
            })
            .collect();
        Self {
            id: id.into(),
            steps,
            active: 0,
            logs: VecDeque::new(),
            visible_lines: 5,
            spinner: Spinner::new(SpinnerStyle::Braille),
        }
    }


    pub fn watching(id: impl Into<String>, task_id: impl Into<TaskId>) -> Self {
        Self::new(id, vec![TaskLogStep::new("", task_id)])
    }

    pub fn with_visible_lines(mut self, n: usize) -> Self {
        self.visible_lines = n.max(1);
        self
    }

    pub fn with_spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner = Spinner::new(style);
        self
    }





    fn active_step(&self) -> Option<&StepState> {
        self.steps.get(self.active)
    }

    fn push_log(&mut self, line: String) {
        self.logs.push_back(line);
        while self.logs.len() > self.visible_lines {
            self.logs.pop_front();
        }
    }



    fn advance(&mut self, succeeded: bool) -> Option<TaskRequest> {
        if let Some(step) = self.steps.get_mut(self.active) {
            step.elapsed_secs = step.started_at.map(|t| t.elapsed().as_secs_f64());
            step.status = if succeeded {
                StepStatus::Done
            } else {
                StepStatus::Error
            };
        }


        self.logs.clear();

        if !succeeded {
            return None;
        }

        let next = self.active + 1;
        if next >= self.steps.len() {
            return None;
        }

        self.active = next;
        if let Some(step) = self.steps.get_mut(self.active) {
            step.status = StepStatus::Running;
            step.started_at = Some(Instant::now());
            Some(TaskRequest::new(step.task_id.clone()))
        } else {
            None
        }
    }

    fn render_step_line(
        step: &StepState,
        index: usize,
        total: usize,
        spinner: &Spinner,
    ) -> Vec<Span> {
        let counter = format!("[{}/{}]", index + 1, total);
        let dim = Style::new().color(Color::DarkGrey);

        match step.status {
            StepStatus::Pending => vec![
                Span::styled(counter, dim).no_wrap(),
                Span::new(" ○ ").no_wrap(),
                Span::styled(step.label.clone(), dim).no_wrap(),
            ],
            StepStatus::Running => {
                let elapsed = step
                    .started_at
                    .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                    .unwrap_or_default();
                vec![
                    Span::styled(counter, Style::new().color(Color::Cyan)).no_wrap(),
                    Span::new(" ").no_wrap(),
                    spinner.span(),
                    Span::new(" ").no_wrap(),
                    Span::styled(
                        format!("{}...", step.label),
                        Style::new().color(Color::White).bold(),
                    )
                    .no_wrap(),
                    Span::styled(elapsed, dim).no_wrap(),
                ]
            }
            StepStatus::Done => {
                let elapsed = step
                    .elapsed_secs
                    .map(|s| format!("  {:.1}s", s))
                    .unwrap_or_default();
                vec![
                    Span::styled(counter, Style::new().color(Color::Green)).no_wrap(),
                    Span::new(" ").no_wrap(),
                    Span::styled("✓", Style::new().color(Color::Green).bold()).no_wrap(),
                    Span::new(" ").no_wrap(),
                    Span::styled(step.label.clone(), dim).no_wrap(),
                    Span::styled(elapsed, dim).no_wrap(),
                ]
            }
            StepStatus::Error => {
                let elapsed = step
                    .elapsed_secs
                    .map(|s| format!("  {:.1}s", s))
                    .unwrap_or_default();
                vec![
                    Span::styled(counter, Style::new().color(Color::Red)).no_wrap(),
                    Span::new(" ").no_wrap(),
                    Span::styled("✗", Style::new().color(Color::Red).bold()).no_wrap(),
                    Span::new(" ").no_wrap(),
                    Span::styled(step.label.clone(), dim).no_wrap(),
                    Span::styled(elapsed, dim).no_wrap(),
                ]
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
                if step.status == StepStatus::Pending {
                    continue;
                }
                lines.push(Self::render_step_line(step, i, total, &self.spinner));
            }
        } else if let Some(step) = self.steps.first() {

            match step.status {
                StepStatus::Running => lines.push(vec![
                    self.spinner.span(),
                    Span::new(" Running...").no_wrap(),
                ]),
                StepStatus::Done => lines.push(vec![
                    Span::styled("✓", Style::new().color(Color::Green).bold()).no_wrap(),
                    Span::new(" Done").no_wrap(),
                ]),
                StepStatus::Error => lines.push(vec![
                    Span::styled("✗", Style::new().color(Color::Red).bold()).no_wrap(),
                    Span::new(" Failed").no_wrap(),
                ]),
                StepStatus::Pending => {}
            }
        }


        let show_logs = self
            .active_step()
            .is_some_and(|s| s.status == StepStatus::Running);
        if show_logs {
            for line in &self.logs {
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
        let running = self
            .active_step()
            .is_some_and(|s| s.status == StepStatus::Running);
        if running {
            self.spinner.tick();
            return InteractionResult::handled();
        }
        InteractionResult::ignored()
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        match event {
            SystemEvent::TaskLogLine { task_id, line } => {
                let is_active = self
                    .active_step()
                    .is_some_and(|s| &s.task_id == task_id && s.status == StepStatus::Running);
                if is_active {
                    self.push_log(line.clone());
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }

            SystemEvent::TaskCompleted { completion } => {
                let is_active = self
                    .active_step()
                    .is_some_and(|s| s.task_id == completion.task_id);
                if !is_active {
                    return InteractionResult::ignored();
                }

                let succeeded = completion.error.is_none() && !completion.cancelled;
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
        if let Some(step) = self.steps.first_mut() {
            step.status = StepStatus::Running;
            Some(TaskRequest::new(step.task_id.clone()))
        } else {
            None
        }
    }
}
