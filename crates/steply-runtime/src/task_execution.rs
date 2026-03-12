use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use steply_core::core::value::Value;
use steply_core::task::execution::{TaskCompletion, TaskInvocation};
use steply_core::task::spec::TaskKind;

pub fn execute_invocation(invocation: TaskInvocation) -> TaskCompletion {
    let task_id = invocation.spec.id.clone();
    let concurrency_policy = invocation.spec.concurrency_policy;

    let TaskKind::Exec {
        program,
        args,
        timeout_ms,
        ..
    } = invocation.spec.kind.clone();

    let mut command = Command::new(program.as_str());
    command
        .args(args.as_slice())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                concurrency_policy,
                result: Value::None,
                error: Some(format!("spawn failed: {err}")),
                cancelled: false,
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = stdin.write_all(invocation.stdin_json.as_bytes())
    {
        let _ = child.kill();
        let _ = child.wait();
        return TaskCompletion {
            task_id,
            run_id: invocation.run_id,
            concurrency_policy,
            result: Value::None,
            error: Some(format!("stdin write failed: {err}")),
            cancelled: false,
        };
    }

    let log_tx = invocation.log_tx.clone();
    let mut stdout_handle = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = BufReader::new(stdout).read_to_string(&mut buf);
            buf
        })
    });
    let mut stderr_handle = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut lines = Vec::new();
            for line in reader.lines() {
                let line = line.unwrap_or_default();
                if let Some(ref tx) = log_tx {
                    let _ = tx.send(line.clone());
                }
                lines.push(line);
            }
            lines.join("\n")
        })
    });

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started_at = Instant::now();
    let status = loop {
        if invocation.cancel_token.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = take_output(&mut stdout_handle);
            let _ = take_output(&mut stderr_handle);
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                concurrency_policy,
                result: Value::None,
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
                    let _ = take_output(&mut stdout_handle);
                    let _ = take_output(&mut stderr_handle);
                    return TaskCompletion {
                        task_id,
                        run_id: invocation.run_id,
                        concurrency_policy,
                        result: Value::None,
                        error: Some(format!("timeout after {}ms", timeout_ms.max(1))),
                        cancelled: false,
                    };
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = take_output(&mut stdout_handle);
                let _ = take_output(&mut stderr_handle);
                return TaskCompletion {
                    task_id,
                    run_id: invocation.run_id,
                    concurrency_policy,
                    result: Value::None,
                    error: Some(format!("wait failed: {err}")),
                    cancelled: false,
                };
            }
        }
    };

    let stdout = take_output(&mut stdout_handle);
    let stderr = take_output(&mut stderr_handle);

    let status_error = if status.success() {
        None
    } else {
        Some(format!(
            "exit status {:?}: {}",
            status.code(),
            normalize_text(stderr.as_str())
        ))
    };

    let result = if status_error.is_none() {
        match parse_task_result(stdout.as_str()) {
            Ok(result) => result,
            Err(err) => {
                return TaskCompletion {
                    task_id,
                    run_id: invocation.run_id,
                    concurrency_policy,
                    result: Value::None,
                    error: Some(err),
                    cancelled: false,
                };
            }
        }
    } else {
        Value::None
    };

    TaskCompletion {
        task_id,
        run_id: invocation.run_id,
        concurrency_policy,
        result,
        error: status_error,
        cancelled: false,
    }
}

fn take_output(handle: &mut Option<JoinHandle<String>>) -> String {
    handle
        .take()
        .and_then(|join| join.join().ok())
        .unwrap_or_default()
}

fn normalize_text(value: &str) -> &str {
    value.trim_end_matches(['\r', '\n'])
}

fn parse_task_result(stdout: &str) -> Result<Value, String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Value::None);
    }
    Value::from_json(trimmed).map_err(|err| format!("invalid JSON task result: {err}"))
}
