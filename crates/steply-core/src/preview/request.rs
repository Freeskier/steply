use serde::{Deserialize, Serialize};

use crate::terminal::TerminalSize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderJsonScope {
    Current,
    Flow,
    Step { step_id: String },
    Widget { step_id: String, widget_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderJsonRequest {
    pub scope: RenderJsonScope,
    pub active_step_id: Option<String>,
    pub terminal_size: Option<TerminalSize>,
}

impl Default for RenderJsonRequest {
    fn default() -> Self {
        Self {
            scope: RenderJsonScope::Current,
            active_step_id: None,
            terminal_size: None,
        }
    }
}

impl RenderJsonScope {
    pub fn from_name(
        scope_name: &str,
        step_id: Option<String>,
        widget_id: Option<String>,
    ) -> Result<Self, String> {
        match scope_name {
            "current" => Ok(Self::Current),
            "flow" => Ok(Self::Flow),
            "step" => {
                let Some(step_id) = step_id else {
                    return Err("scope 'step' requires step_id".to_string());
                };
                Ok(Self::Step { step_id })
            }
            "widget" => {
                let Some(step_id) = step_id else {
                    return Err("scope 'widget' requires step_id".to_string());
                };
                let Some(widget_id) = widget_id else {
                    return Err("scope 'widget' requires widget_id".to_string());
                };
                Ok(Self::Widget { step_id, widget_id })
            }
            other => Err(format!(
                "unsupported scope: {} (expected current|flow|step|widget)",
                other
            )),
        }
    }
}

impl RenderJsonRequest {
    pub fn from_named_parts(
        scope_raw: Option<String>,
        step_id: Option<String>,
        widget_id: Option<String>,
        active_step_id: Option<String>,
        width: Option<u16>,
        height: Option<u16>,
    ) -> Result<Self, String> {
        let scope_name = scope_raw.unwrap_or_else(|| "current".to_string());
        let scope = RenderJsonScope::from_name(scope_name.as_str(), step_id, widget_id)?;
        let terminal_size = match (width, height) {
            (Some(width), Some(height)) => Some(TerminalSize { width, height }),
            (None, None) => None,
            _ => return Err("width and height must be provided together".to_string()),
        };

        Ok(Self {
            scope,
            active_step_id,
            terminal_size,
        })
    }
}
