use std::env;
use std::io;
use std::path::PathBuf;

use crate::config::load_from_yaml_file;
use crate::runtime::{RenderJsonRequest, RenderJsonScope, Runtime};
use crate::state::app::AppState;
use crate::state::demo::{build_demo_flow, build_demo_tasks};
use crate::terminal::{RenderMode, Terminal, TerminalSize};
use crate::ui::renderer::RendererConfig;

#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    pub config_path: Option<PathBuf>,
    pub render_json: Option<RenderJsonRequest>,
}

impl StartOptions {
    pub fn from_env() -> Result<Self, String> {
        let args = env::args().collect::<Vec<_>>();
        let mut idx = 1usize;

        let mut config_path = None::<PathBuf>;
        let mut render_json_requested = false;
        let mut scope = None::<String>;
        let mut step_id = None::<String>;
        let mut widget_id = None::<String>;
        let mut active_step_id = None::<String>;
        let mut width = None::<u16>;
        let mut height = None::<u16>;

        while idx < args.len() {
            match args[idx].as_str() {
                "--config" if idx + 1 < args.len() => {
                    config_path = Some(PathBuf::from(args[idx + 1].clone()));
                    idx += 1;
                }
                "--render-json" => {
                    render_json_requested = true;
                }
                "--render-scope" if idx + 1 < args.len() => {
                    scope = Some(args[idx + 1].clone());
                    idx += 1;
                }
                "--render-step-id" if idx + 1 < args.len() => {
                    step_id = Some(args[idx + 1].clone());
                    idx += 1;
                }
                "--render-widget-id" if idx + 1 < args.len() => {
                    widget_id = Some(args[idx + 1].clone());
                    idx += 1;
                }
                "--render-active-step-id" if idx + 1 < args.len() => {
                    active_step_id = Some(args[idx + 1].clone());
                    idx += 1;
                }
                "--render-width" if idx + 1 < args.len() => {
                    width = Some(parse_u16_arg(args[idx + 1].as_str(), "--render-width")?);
                    idx += 1;
                }
                "--render-height" if idx + 1 < args.len() => {
                    height = Some(parse_u16_arg(args[idx + 1].as_str(), "--render-height")?);
                    idx += 1;
                }
                "--config"
                | "--render-scope"
                | "--render-step-id"
                | "--render-widget-id"
                | "--render-active-step-id"
                | "--render-width"
                | "--render-height" => {
                    return Err(format!("missing value for argument: {}", args[idx]));
                }
                _ => {}
            }
            idx += 1;
        }

        let render_json = if render_json_requested {
            Some(build_render_request(
                scope,
                step_id,
                widget_id,
                active_step_id,
                width,
                height,
            )?)
        } else {
            None
        };

        Ok(Self {
            config_path,
            render_json,
        })
    }
}

pub fn run_with_options(options: StartOptions) -> io::Result<()> {
    let (flow, task_specs, task_subscriptions) = if let Some(config_path) = options.config_path {
        let loaded = load_from_yaml_file(config_path.as_path())
            .map_err(|err| io::Error::other(format!("yaml config error: {err}")))?;
        (loaded.flow, loaded.task_specs, loaded.task_subscriptions)
    } else {
        let flow = build_demo_flow();
        let (task_specs, task_subscriptions) = build_demo_tasks();
        (flow, task_specs, task_subscriptions)
    };

    let state = AppState::with_tasks(flow, task_specs, task_subscriptions);
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

fn build_render_request(
    scope_raw: Option<String>,
    step_id: Option<String>,
    widget_id: Option<String>,
    active_step_id: Option<String>,
    width: Option<u16>,
    height: Option<u16>,
) -> Result<RenderJsonRequest, String> {
    let scope_name = scope_raw.unwrap_or_else(|| "current".to_string());
    let scope = match scope_name.as_str() {
        "current" => RenderJsonScope::Current,
        "flow" => RenderJsonScope::Flow,
        "step" => {
            let Some(step_id) = step_id else {
                return Err("--render-scope step requires --render-step-id <id>".to_string());
            };
            RenderJsonScope::Step { step_id }
        }
        "widget" => {
            let Some(step_id) = step_id else {
                return Err("--render-scope widget requires --render-step-id <id>".to_string());
            };
            let Some(widget_id) = widget_id else {
                return Err("--render-scope widget requires --render-widget-id <id>".to_string());
            };
            RenderJsonScope::Widget { step_id, widget_id }
        }
        other => {
            return Err(format!(
                "unsupported --render-scope value: {} (expected current|flow|step|widget)",
                other
            ));
        }
    };

    let terminal_size = match (width, height) {
        (Some(width), Some(height)) => Some(TerminalSize { width, height }),
        (Some(_), None) | (None, Some(_)) => {
            return Err("--render-width and --render-height must be provided together".to_string());
        }
        (None, None) => None,
    };

    Ok(RenderJsonRequest {
        scope,
        active_step_id,
        terminal_size,
    })
}

fn parse_u16_arg(raw: &str, arg_name: &str) -> Result<u16, String> {
    raw.parse::<u16>()
        .map_err(|_| format!("invalid value for {}: {}", arg_name, raw))
}
