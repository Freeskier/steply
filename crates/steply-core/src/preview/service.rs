use crate::config::{ConfigLoadError, LoadedConfig, load_from_yaml_str};
use crate::preview::render::render_json;
use crate::preview::request::RenderJsonRequest;
use crate::state::app::AppStateInitError;
use crate::state::flow::Flow;
use crate::task::{TaskSpec, TaskSubscription};
use crate::terminal::TerminalSize;
use crate::ui::renderer::{Renderer, RendererConfig};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewServiceOptions {
    pub default_terminal_size: TerminalSize,
    pub chrome_enabled: bool,
}

impl Default for PreviewServiceOptions {
    fn default() -> Self {
        Self {
            default_terminal_size: TerminalSize {
                width: 100,
                height: 40,
            },
            chrome_enabled: true,
        }
    }
}

pub struct PreviewService {
    state: crate::state::app::AppState,
    renderer: Renderer,
    default_terminal_size: TerminalSize,
}

#[derive(Debug)]
pub enum PreviewServiceInitError {
    Config(ConfigLoadError),
    AppState(AppStateInitError),
}

impl fmt::Display for PreviewServiceInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(err) => write!(f, "{err}"),
            Self::AppState(err) => write!(f, "{err}"),
        }
    }
}

impl Error for PreviewServiceInitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(err) => Some(err),
            Self::AppState(err) => Some(err),
        }
    }
}

impl From<ConfigLoadError> for PreviewServiceInitError {
    fn from(value: ConfigLoadError) -> Self {
        Self::Config(value)
    }
}

impl From<AppStateInitError> for PreviewServiceInitError {
    fn from(value: AppStateInitError) -> Self {
        Self::AppState(value)
    }
}

impl PreviewService {
    pub fn from_loaded_config(loaded: LoadedConfig) -> Result<Self, AppStateInitError> {
        Self::from_loaded_config_with_options(loaded, PreviewServiceOptions::default())
    }

    pub fn from_loaded_config_with_options(
        loaded: LoadedConfig,
        options: PreviewServiceOptions,
    ) -> Result<Self, AppStateInitError> {
        let state = loaded.into_app_state()?;
        Ok(Self {
            state,
            renderer: Renderer::new(RendererConfig {
                chrome_enabled: options.chrome_enabled,
            }),
            default_terminal_size: options.default_terminal_size,
        })
    }

    pub fn from_flow(flow: Flow) -> Result<Self, AppStateInitError> {
        Self::from_parts(
            flow,
            Vec::new(),
            Vec::new(),
            PreviewServiceOptions::default(),
        )
    }

    pub fn from_parts(
        flow: Flow,
        task_specs: Vec<TaskSpec>,
        task_subscriptions: Vec<TaskSubscription>,
        options: PreviewServiceOptions,
    ) -> Result<Self, AppStateInitError> {
        let loaded = LoadedConfig {
            flow,
            task_specs,
            task_subscriptions,
        };
        Self::from_loaded_config_with_options(loaded, options)
    }

    pub fn from_yaml_str(raw: &str) -> Result<Self, PreviewServiceInitError> {
        Self::from_yaml_str_with_options(raw, PreviewServiceOptions::default())
    }

    pub fn from_yaml_str_with_options(
        raw: &str,
        options: PreviewServiceOptions,
    ) -> Result<Self, PreviewServiceInitError> {
        let loaded = load_from_yaml_str(raw)?;
        Self::from_loaded_config_with_options(loaded, options).map_err(Into::into)
    }

    pub fn render(&mut self, request: &RenderJsonRequest) -> Result<serde_json::Value, String> {
        render_json(
            &mut self.state,
            request,
            &mut self.renderer,
            self.default_terminal_size,
        )
    }
}

pub fn render_yaml_preview_json(
    yaml: &str,
    request: &RenderJsonRequest,
    options: PreviewServiceOptions,
) -> Result<serde_json::Value, String> {
    let mut service =
        PreviewService::from_yaml_str_with_options(yaml, options).map_err(|err| err.to_string())?;
    service.render(request)
}
