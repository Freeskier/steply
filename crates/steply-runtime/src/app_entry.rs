use std::io;
use std::io::Read;
use std::path::PathBuf;

use crate::terminal::{RenderMode, Terminal};
use crate::{RenderJsonRequest, Runtime};
use steply_core::config::{load_from_yaml_file, load_from_yaml_str};
use steply_core::state::demo::{build_demo_flow, build_demo_tasks};
use steply_core::ui::renderer::RendererConfig;
use steply_core::{HostContext, set_host_context};

#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    pub config_path: Option<String>,
    pub render_json: Option<RenderJsonRequest>,
}

pub fn run_with_options(options: StartOptions) -> io::Result<()> {
    let _ = set_host_context(HostContext {
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        home_dir: std::env::var_os("HOME").map(PathBuf::from),
    });

    let state = if let Some(config_path) = options.config_path {
        let loaded = if config_path == "-" {
            let mut raw = String::new();
            io::stdin().read_to_string(&mut raw)?;
            load_from_yaml_str(raw.as_str())
        } else if is_http_url(config_path.as_str()) {
            let raw = fetch_remote_config(config_path.as_str())?;
            load_from_yaml_str(raw.as_str())
        } else {
            load_from_yaml_file(PathBuf::from(config_path).as_path())
        }
        .map_err(|err| io::Error::other(format!("yaml config error: {err}")))?;
        loaded
            .into_app_state()
            .map_err(|err| io::Error::other(format!("app init error: {err}")))?
    } else {
        let flow = build_demo_flow();
        let task_specs = build_demo_tasks();
        steply_core::state::app::AppState::with_tasks(flow, task_specs)
            .map_err(|err| io::Error::other(format!("app init error: {err}")))?
    };
    let terminal = Terminal::new()?;
    let mut runtime = Runtime::new(state, terminal)
        .with_render_mode(RenderMode::AltScreen)
        .with_renderer_config(RendererConfig {
            chrome_enabled: true,
        });

    if let Some(request) = options.render_json {
        return runtime.print_render_json_with_request(request);
    }

    runtime.run()
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn fetch_remote_config(url: &str) -> io::Result<String> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| io::Error::other(format!("failed to fetch config from URL: {err}")))?;
    response
        .into_string()
        .map_err(|err| io::Error::other(format!("failed to read config response: {err}")))
}
