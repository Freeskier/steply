use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::task::{TaskId, TaskRequest};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::spinner::{Spinner, SpinnerStyle};
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem, InteractionResult,
    Interactive, RenderContext,
};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunStatus {
    Idle,
    Running,
    Done,
    Error,
}

pub struct CommandRunner {
    base: WidgetBase,
    task_id: TaskId,
    status: RunStatus,
    logs: VecDeque<String>,
    visible_lines: usize,
    spinner: Spinner,
    children: Vec<Node>,
}

impl CommandRunner {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        task_id: impl Into<TaskId>,
    ) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            task_id: task_id.into(),
            status: RunStatus::Idle,
            logs: VecDeque::new(),
            visible_lines: 6,
            spinner: Spinner::new(SpinnerStyle::Squares),
            children: Vec::new(),
        }
    }

    pub fn with_visible_lines(mut self, n: usize) -> Self {
        self.visible_lines = n.max(1);
        self
    }

    pub fn with_spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner = Spinner::new(style);
        self
    }

    fn push_log(&mut self, line: String) {
        self.logs.push_back(line);
        while self.logs.len() > self.visible_lines {
            self.logs.pop_front();
        }
    }

    fn status_line(&self, focused: bool) -> Vec<Span> {
        match self.status {
            RunStatus::Idle => vec![
                Span::styled(
                    "Press Enter to run",
                    if focused {
                        Style::new().color(Color::Cyan)
                    } else {
                        Style::new().color(Color::DarkGrey)
                    },
                )
                .no_wrap(),
            ],
            RunStatus::Running => vec![
                Span::styled(
                    self.spinner.glyph().to_string(),
                    Style::new().color(Color::Blue).bold(),
                )
                .no_wrap(),
                Span::styled(" Running...", Style::new().color(Color::Blue)).no_wrap(),
            ],
            RunStatus::Done => vec![
                Span::styled("✓", Style::new().color(Color::Green).bold()).no_wrap(),
                Span::styled(" Done", Style::new().color(Color::DarkGrey)).no_wrap(),
            ],
            RunStatus::Error => vec![
                Span::styled("✗", Style::new().color(Color::Red).bold()).no_wrap(),
                Span::styled(" Failed", Style::new().color(Color::DarkGrey)).no_wrap(),
            ],
        }
    }
}

impl Component for CommandRunner {
    fn children(&self) -> &[Node] {
        self.children.as_slice()
    }

    fn children_mut(&mut self) -> &mut [Node] {
        self.children.as_mut_slice()
    }
}

impl Drawable for CommandRunner {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let title_style = if focused {
            Style::new().color(Color::White)
        } else {
            Style::new().color(Color::DarkGrey)
        };

        let mut lines = vec![
            vec![Span::styled(self.base.label().to_string(), title_style).no_wrap()],
            self.status_line(focused),
        ];

        for line in &self.logs {
            lines.push(vec![
                Span::styled(format!("  {line}"), Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        DrawOutput { lines }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }
        vec![HintItem::new("Enter", "run command", HintGroup::Action).with_priority(20)]
    }
}

impl Interactive for CommandRunner {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                if self.status == RunStatus::Running {
                    return InteractionResult::handled();
                }
                self.logs.clear();
                self.status = RunStatus::Running;
                InteractionResult::with_action(WidgetAction::TaskRequested {
                    request: TaskRequest::new(self.task_id.clone()),
                })
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        match event {
            SystemEvent::TaskLogLine { task_id, line } if task_id == &self.task_id => {
                self.push_log(line.clone());
                InteractionResult::handled()
            }
            SystemEvent::TaskCompleted { completion } if completion.task_id == self.task_id => {
                self.status = if completion.error.is_none() && !completion.cancelled {
                    RunStatus::Done
                } else {
                    RunStatus::Error
                };
                InteractionResult::handled()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_tick(&mut self) -> InteractionResult {
        if self.status == RunStatus::Running {
            self.spinner.tick();
            return InteractionResult::handled();
        }
        InteractionResult::ignored()
    }
}
