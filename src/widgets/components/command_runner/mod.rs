use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::task::{TaskId, TaskSpec};
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::spinner::SpinnerStyle;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{Component, Node};
use crate::widgets::outputs::task_log::{TaskLog, TaskLogStep};
use crate::widgets::shared::task_watcher::TaskWatcherStatus;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem, InteractionResult,
    Interactive, OutputNode, RenderContext, ValidationMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    Manual,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnError {
    #[default]
    Stay,
    Continue,
}

struct CommandSpec {
    label: String,
    task_id: TaskId,
    program: String,
    args: Vec<String>,
    timeout_ms: u64,
}

pub struct CommandRunner {
    base: WidgetBase,
    commands: Vec<CommandSpec>,
    log: TaskLog,
    last_error: Option<String>,
    run_mode: RunMode,
    advance_on_success: bool,
    on_error: OnError,
    auto_run_armed: bool,
    children: Vec<Node>,
}

impl CommandRunner {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let label = label.into();
        Self {
            base: WidgetBase::new(id.clone(), label),
            commands: Vec::new(),
            log: TaskLog::new(format!("{id}__log"), Vec::new())
                .with_spinner_style(SpinnerStyle::Dots)
                .with_visible_lines(6),
            last_error: None,
            run_mode: RunMode::default(),
            advance_on_success: false,
            on_error: OnError::default(),
            auto_run_armed: true,
            children: Vec::new(),
        }
    }

    pub fn command<I, S>(
        mut self,
        label: impl Into<String>,
        program: impl Into<String>,
        args: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let index = self.commands.len();
        let task_id = TaskId::new(format!("{}::command::{index}", self.base.id()));
        let label = label.into();

        self.log
            .push_step(TaskLogStep::new(label.clone(), task_id.clone()));
        self.commands.push(CommandSpec {
            label,
            task_id,
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            timeout_ms: 30_000,
        });
        self
    }

    pub fn with_visible_lines(mut self, n: usize) -> Self {
        self.log = self.log.with_visible_lines(n);
        self
    }

    pub fn with_spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.log = self.log.with_spinner_style(style);
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        let timeout_ms = timeout_ms.max(1);
        for command in &mut self.commands {
            command.timeout_ms = timeout_ms;
        }
        self
    }

    pub fn with_run_mode(mut self, mode: RunMode) -> Self {
        self.run_mode = mode;
        self
    }

    pub fn with_advance_on_success(mut self, enabled: bool) -> Self {
        self.advance_on_success = enabled;
        self
    }

    pub fn with_on_error(mut self, on_error: OnError) -> Self {
        self.on_error = on_error;
        self
    }

    fn status_line(&self, focused: bool) -> Option<Vec<Span>> {
        match self.log.status() {
            TaskWatcherStatus::Idle => Some(vec![
                Span::styled(
                    if self.run_mode == RunMode::Auto {
                        "Runs automatically on step enter (Enter to rerun)"
                    } else {
                        "Press Enter to run"
                    },
                    if focused {
                        Style::new().color(Color::Cyan)
                    } else {
                        Style::new().color(Color::DarkGrey)
                    },
                )
                .no_wrap(),
            ]),
            TaskWatcherStatus::Pending
            | TaskWatcherStatus::Running
            | TaskWatcherStatus::Done
            | TaskWatcherStatus::Error => None,
        }
    }

    fn command_by_task_id(&self, task_id: &TaskId) -> Option<&CommandSpec> {
        self.commands
            .iter()
            .find(|command| &command.task_id == task_id)
    }

    fn can_start(&self) -> bool {
        !self.commands.is_empty()
            && !matches!(
                self.log.status(),
                TaskWatcherStatus::Running | TaskWatcherStatus::Pending
            )
    }

    fn start_run(&mut self) -> InteractionResult {
        if !self.can_start() {
            return InteractionResult::handled();
        }
        self.last_error = None;
        if let Some(request) = self.log.start_request() {
            let mut result =
                InteractionResult::with_action(WidgetAction::TaskRequested { request });
            result.actions.push(WidgetAction::ValidateCurrentStepSubmit);
            return result;
        }
        InteractionResult::handled()
    }

    fn maybe_strip_input_done(
        mut result: InteractionResult,
        advance_on_success: bool,
    ) -> InteractionResult {
        if !advance_on_success {
            result
                .actions
                .retain(|action| !matches!(action, WidgetAction::InputDone));
        }
        result
    }

    fn task_failure_message(completion: &crate::task::TaskCompletion) -> String {
        if completion.cancelled {
            return "cancelled".to_string();
        }

        if let Some(line) = completion
            .stderr
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
        {
            return line.to_string();
        }

        if let Some(line) = completion
            .stdout
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
        {
            return line.to_string();
        }

        if let Some(code) = completion.status_code {
            return format!("exit status {code}");
        }

        completion
            .error
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("failed")
            .to_string()
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

        let mut lines = vec![vec![
            Span::styled(self.base.label().to_string(), title_style).no_wrap(),
        ]];

        if self.commands.is_empty() {
            lines.push(vec![
                Span::styled(
                    "  no commands configured",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]);
            return DrawOutput { lines };
        }

        if let Some(status_line) = self.status_line(focused) {
            lines.push(status_line);
        }

        lines.extend(self.log.draw(ctx).lines);
        DrawOutput { lines }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }
        vec![
            HintItem::new(
                "Enter",
                if self.run_mode == RunMode::Auto {
                    "rerun command"
                } else {
                    "run command"
                },
                HintGroup::Action,
            )
            .with_priority(20),
        ]
    }
}

impl Interactive for CommandRunner {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Enter => {
                self.auto_run_armed = false;
                self.start_run()
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_system_event(&mut self, event: &SystemEvent) -> InteractionResult {
        let mut should_validate_step = false;
        let mut should_continue_on_error = false;
        let mut auto_start = false;
        match event {
            SystemEvent::RequestFocus { target } => {
                if target.as_str() == self.base.id() {
                    if self.run_mode == RunMode::Auto && self.auto_run_armed && self.can_start() {
                        self.auto_run_armed = false;
                        auto_start = true;
                    }
                } else {
                    self.auto_run_armed = true;
                }
            }
            SystemEvent::TaskStartRejected { task_id, reason } => {
                if let Some(command) = self.command_by_task_id(task_id) {
                    self.last_error = Some(format!("{}: {reason}", command.label));
                    should_validate_step = true;
                    should_continue_on_error = true;
                }
            }
            SystemEvent::TaskCompleted { completion } => {
                if let Some(command) = self.command_by_task_id(&completion.task_id)
                    && (completion.cancelled || completion.error.is_some())
                {
                    let message = Self::task_failure_message(completion);
                    self.last_error = Some(format!("{}: {message}", command.label));
                    should_validate_step = true;
                    should_continue_on_error = true;
                }
            }
            _ => {}
        }
        let mut result =
            Self::maybe_strip_input_done(self.log.on_system_event(event), self.advance_on_success);
        if self.on_error == OnError::Continue && should_continue_on_error {
            result.actions.push(WidgetAction::InputDone);
            result.handled = true;
            result.request_render = true;
            should_validate_step = false;
        }
        if should_validate_step {
            result.actions.push(WidgetAction::ValidateCurrentStepSubmit);
            result.handled = true;
            result.request_render = true;
        }
        if auto_start {
            result.merge(self.start_run());
        }
        result
    }

    fn on_tick(&mut self) -> InteractionResult {
        self.log.on_tick()
    }

    fn task_specs(&self) -> Vec<TaskSpec> {
        self.commands
            .iter()
            .map(|command| {
                TaskSpec::exec(
                    command.task_id.clone(),
                    command.program.clone(),
                    command.args.clone(),
                )
                .with_timeout_ms(command.timeout_ms)
            })
            .collect()
    }

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        if mode == ValidationMode::Submit
            && self.on_error != OnError::Continue
            && let Some(error) = &self.last_error
        {
            return Err(error.clone());
        }
        Ok(())
    }
}
