use crate::core::value::Value;
use crate::task::policy::ConcurrencyPolicy;
use crate::task::spec::{TaskAssign, TaskId, TaskKind, TaskParse, TaskSpec};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
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
    pub cancel_token: TaskCancelToken,
    pub log_tx: Option<Sender<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskCancelToken {
    cancelled: Arc<AtomicBool>,
}

impl TaskCancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
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
    pub cancelled: bool,
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
                cancelled: false,
            };
        }
    };

    // Stream stdout line-by-line in a background thread, forwarding each line
    // through log_tx (if any) and collecting them for the final TaskCompletion.
    let stdout_reader = child.stdout.take().map(BufReader::new);
    let log_tx = invocation.log_tx.clone();
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<String>();
    let reader_handle = if let Some(reader) = stdout_reader {
        Some(std::thread::spawn(move || {
            for line in reader.lines() {
                let line = line.unwrap_or_default();
                if let Some(ref tx) = log_tx {
                    let _ = tx.send(line.clone());
                }
                let _ = lines_tx.send(line);
            }
        }))
    } else {
        None
    };

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started_at = Instant::now();
    let status = loop {
        if invocation.cancel_token.is_cancelled() {
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
                error: Some("cancelled".to_string()),
                cancelled: true,
            };
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
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
                        cancelled: false,
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
                    cancelled: false,
                };
            }
        }
    };

    // Wait for the reader thread to finish draining the pipe before collecting.
    if let Some(handle) = reader_handle {
        let _ = handle.join();
    }
    let stdout: String = lines_rx.try_iter().collect::<Vec<_>>().join("\n");

    let stderr = child
        .stderr
        .take()
        .map(|s| {
            let mut buf = String::new();
            use std::io::Read;
            BufReader::new(s).read_to_string(&mut buf).ok();
            buf
        })
        .unwrap_or_default();

    let status_code = status.code();

    let parse_result = parse_value(invocation.spec.parse, stdout.as_str());
    let (value, parse_error) = match parse_result {
        Ok(value) => (Some(value), None),
        Err(err) => (None, Some(err)),
    };

    let status_error = if status.success() {
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
        cancelled: false,
    }
}

fn parse_value(parse: TaskParse, stdout: &str) -> Result<Value, String> {
    let normalized = normalize_text(stdout);

    match parse {
        TaskParse::Number => parse_number(normalized),
        TaskParse::RawText => Ok(Value::Text(normalized.to_string())),
        TaskParse::Json => Value::from_json(normalized),
        TaskParse::Regex { pattern, group } => parse_regex(normalized, &pattern, group),
        TaskParse::Lines => {
            let lines = normalized
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| Value::Text(line.to_string()))
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
        return Ok(Value::Number(number));
    }

    Err(format!("number parse error: '{first}'"))
}

fn parse_regex(value: &str, pattern: &str, group: usize) -> Result<Value, String> {
    let re = regex::Regex::new(pattern).map_err(|e| format!("bad regex: {e}"))?;
    let caps = re
        .captures(value)
        .ok_or_else(|| format!("no match for /{pattern}/"))?;
    let matched = caps
        .get(group)
        .ok_or_else(|| format!("no group {group} in match"))?
        .as_str();
    Ok(Value::Text(matched.to_string()))
}
