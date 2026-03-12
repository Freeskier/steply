use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use steply_core::task::execution::{TaskCompletion, TaskInvocation};
use steply_core::task::spec::TaskKind;

pub fn execute_invocation(invocation: TaskInvocation) -> TaskCompletion {
    let task_id = invocation.spec.id.clone();
    let concurrency_policy = invocation.spec.concurrency_policy;

    let TaskKind::Exec {
        program,
        args,
        env,
        timeout_ms,
    } = invocation.spec.kind.clone();

    let mut command = Command::new(program.as_str());
    command
        .args(args.as_slice())
        .envs(env.iter())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                concurrency_policy,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("spawn failed: {err}")),
                cancelled: false,
            };
        }
    };

    let log_tx = invocation.log_tx.clone();
    let mut stdout_handle = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
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
    let mut stderr_handle = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut buf);
            buf
        })
    });

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started_at = Instant::now();
    let status = loop {
        if invocation.cancel_token.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = take_output(&mut stdout_handle);
            let stderr = take_output(&mut stderr_handle);
            return TaskCompletion {
                task_id,
                run_id: invocation.run_id,
                concurrency_policy,
                status_code: None,
                stdout,
                stderr,
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
                    let stdout = take_output(&mut stdout_handle);
                    let stderr = take_output(&mut stderr_handle);
                    return TaskCompletion {
                        task_id,
                        run_id: invocation.run_id,
                        concurrency_policy,
                        status_code: None,
                        stdout,
                        stderr,
                        error: Some(format!("timeout after {}ms", timeout_ms.max(1))),
                        cancelled: false,
                    };
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = take_output(&mut stdout_handle);
                let stderr = take_output(&mut stderr_handle);
                return TaskCompletion {
                    task_id,
                    run_id: invocation.run_id,
                    concurrency_policy,
                    status_code: None,
                    stdout,
                    stderr,
                    error: Some(format!("wait failed: {err}")),
                    cancelled: false,
                };
            }
        }
    };

    let stdout = take_output(&mut stdout_handle);
    let stderr = take_output(&mut stderr_handle);

    let status_code = status.code();

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
        concurrency_policy,
        status_code,
        stdout,
        stderr,
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
