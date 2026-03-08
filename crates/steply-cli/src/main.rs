mod cli;
mod flow;
mod prompt;

use std::backtrace::Backtrace;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use cli::Invocation;
use flow::handle_flow;
use prompt::PromptExit;
use steply_core::config::{config_schema_json, schema_docs_json};
use steply_runtime::run_with_options;

fn main() {
    install_panic_logging();
    if let Err(err) = run() {
        append_error_log(
            error_log_path().as_path(),
            "runtime_error",
            err.message.as_str(),
        );
        if !err.message.is_empty() {
            eprintln!("{}", err.message);
        }
        process::exit(err.exit_code);
    }
}

fn run() -> Result<(), CliError> {
    match cli::parse_invocation() {
        Ok(Invocation::Run(options)) => run_with_options(options).map_err(CliError::io),
        Ok(Invocation::Prompt(invocation)) => {
            if let Some(flow_id) = invocation.flow_id.as_deref() {
                flow::append_widget_to_flow(flow_id, &invocation.doc, &invocation.values)
                    .map_err(|err| CliError::new(1, format!("error: {err}")))
            } else {
                match prompt::run_prompt(invocation) {
                    Ok(PromptExit::Submitted) => Ok(()),
                    Ok(PromptExit::Cancelled) => Err(CliError::new(130, "prompt cancelled")),
                    Err(err) => Err(CliError::new(1, format!("error: {err}"))),
                }
            }
        }
        Ok(Invocation::Export(invocation)) => export_json(invocation).map_err(CliError::io),
        Ok(Invocation::Flow(invocation)) => {
            handle_flow(invocation).map_err(|err| CliError::new(1, format!("error: {err}")))
        }
        Err(err) => {
            let exit_code = err.exit_code();
            err.print().ok();
            Err(CliError::new(exit_code, String::new()))
        }
    }
}

fn error_log_path() -> PathBuf {
    std::env::var_os("STEPLY_ERROR_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/steply-errors.log"))
}

fn install_panic_logging() {
    let log_path = error_log_path();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log_panic(log_path.as_path(), info);
        default_hook(info);
    }));
}

fn log_panic(path: &Path, info: &PanicHookInfo<'_>) {
    let payload = if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    };
    let location = info
        .location()
        .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
        .unwrap_or_else(|| "unknown location".to_string());
    let backtrace = Backtrace::force_capture();
    let body = format!("panic at {location}: {payload}\nbacktrace:\n{backtrace}");
    append_error_log(path, "panic", body.as_str());
}

fn append_error_log(path: &Path, kind: &str, message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "[{timestamp}] {kind}: {message}");
}

fn export_json(invocation: cli::ExportInvocation) -> std::io::Result<()> {
    let json = match invocation.kind {
        cli::ExportKind::Schema => config_schema_json()
            .map_err(|err| std::io::Error::other(format!("schema error: {err}")))?,
        cli::ExportKind::Docs => {
            schema_docs_json().map_err(|err| std::io::Error::other(format!("docs error: {err}")))?
        }
    };

    if let Some(parent) = invocation.out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(invocation.out_path, json)
}

struct CliError {
    exit_code: i32,
    message: String,
}

impl CliError {
    fn new(exit_code: i32, message: impl Into<String>) -> Self {
        Self {
            exit_code,
            message: message.into(),
        }
    }

    fn io(err: std::io::Error) -> Self {
        Self::new(1, format!("error: {err}"))
    }
}
