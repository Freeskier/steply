use crate::core::value::Value;
use crate::task::policy::ConcurrencyPolicy;
use crate::task::spec::{TaskAssign, TaskId, TaskKind, TaskParse, TaskSpec};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub task_id: TaskId,
    pub fingerprint: Option<u64>,
    pub interval: Option<TaskIntervalRequest>,
}

#[derive(Debug, Clone)]
pub struct TaskIntervalRequest {
    pub key: String,
    pub every_ms: u64,
    pub only_when_step_active: bool,
}

impl TaskRequest {
    pub fn new(task_id: impl Into<TaskId>) -> Self {
        Self {
            task_id: task_id.into(),
            fingerprint: None,
            interval: None,
        }
    }

    pub fn with_fingerprint(mut self, fingerprint: u64) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    pub fn with_interval(
        mut self,
        key: impl Into<String>,
        every_ms: u64,
        only_when_step_active: bool,
    ) -> Self {
        self.interval = Some(TaskIntervalRequest {
            key: key.into(),
            every_ms: every_ms.max(1),
            only_when_step_active,
        });
        self
    }
}

#[derive(Debug, Clone)]
pub struct TaskInvocation {
    pub spec: TaskSpec,
    pub run_id: u64,
    pub fingerprint: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TaskCompletion {
    pub task_id: TaskId,
    pub run_id: u64,
    pub assign: TaskAssign,
    pub concurrency_policy: ConcurrencyPolicy,
    pub value: Option<Value>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

pub fn execute_invocation(invocation: TaskInvocation) -> TaskCompletion {
    let task_id = invocation.spec.id.clone();
    let assign = invocation.spec.assign.clone();
    let concurrency_policy = invocation.spec.concurrency_policy;

    let TaskKind::Exec {
        program,
        args,
        timeout_ms,
    } = invocation.spec.kind.clone();

    let mut child = match Command::new(program.as_str())
        .args(args.as_slice())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                assign,
                concurrency_policy,
                value: None,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("spawn failed: {err}")),
            };
        }
    };

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return TaskCompletion {
                        task_id,
                        run_id: invocation.run_id,
                        assign,
                        concurrency_policy,
                        value: None,
                        status_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        error: Some(format!("timeout after {}ms", timeout_ms.max(1))),
                    };
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                return TaskCompletion {
                    task_id,
                    run_id: invocation.run_id,
                    assign,
                    concurrency_policy,
                    value: None,
                    status_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: Some(format!("wait failed: {err}")),
                };
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(err) => {
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                assign,
                concurrency_policy,
                value: None,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("read output failed: {err}")),
            };
        }
    };

    let stdout = String::from_utf8_lossy(output.stdout.as_slice()).to_string();
    let stderr = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
    let status_code = output.status.code();

    let parse_result = parse_value(invocation.spec.parse, stdout.as_str());
    let (value, parse_error) = match parse_result {
        Ok(value) => (Some(value), None),
        Err(err) => (None, Some(err)),
    };

    let status_error = if output.status.success() {
        None
    } else {
        Some(format!(
            "exit status {:?}: {}",
            status_code,
            normalize_text(stderr.as_str())
        ))
    };

    TaskCompletion {
        task_id,
        run_id: invocation.run_id,
        assign,
        concurrency_policy,
        value,
        status_code,
        stdout,
        stderr,
        error: status_error.or(parse_error),
    }
}

fn parse_value(parse: TaskParse, stdout: &str) -> Result<Value, String> {
    let normalized = normalize_text(stdout);

    match parse {
        TaskParse::Number => parse_number(normalized),
        TaskParse::RawText | TaskParse::Json | TaskParse::Regex { .. } => {
            Ok(Value::Text(normalized.to_string()))
        }
        TaskParse::Lines => {
            let lines = normalized
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            Ok(Value::List(lines))
        }
    }
}

fn normalize_text(value: &str) -> &str {
    value.trim_end_matches(['\r', '\n'])
}

fn parse_number(value: &str) -> Result<Value, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("number parse error: empty output".to_string());
    }

    let first = trimmed
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches('%');

    if let Ok(number) = first.parse::<f64>() {
        return Ok(Value::Float(number));
    }

    Err(format!("number parse error: '{first}'"))
}
