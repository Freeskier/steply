use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use steply_v2::runtime::Runtime;
use steply_v2::state::app::AppState;
use steply_v2::state::demo::{build_demo_flow, build_demo_tasks};
use steply_v2::terminal::Terminal;

fn main() {
    install_panic_logging();
    if let Err(err) = run() {
        append_error_log(
            error_log_path().as_path(),
            "runtime_error",
            format!("error: {err}").as_str(),
        );
        eprintln!("error: {err}");
    }
}

fn run() -> std::io::Result<()> {
    let flow = build_demo_flow();
    let (task_specs, task_subscriptions) = build_demo_tasks();
    let state = AppState::with_tasks(flow, task_specs, task_subscriptions);
    let terminal = Terminal::new()?;
    let mut runtime = Runtime::new(state, terminal);
    if render_json_enabled() {
        return runtime.print_render_json();
    }
    runtime.run()
}

fn render_json_enabled() -> bool {
    std::env::var("STEPLY_RENDER_JSON")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
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
