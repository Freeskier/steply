use serde::{Deserialize, Serialize};
use steply_core::preview::{RenderJsonRequest, RenderJsonScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPreviewRequest {
    pub scope: String,
    pub step_id: Option<String>,
    pub widget_id: Option<String>,
    pub active_step_id: Option<String>,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

impl TryFrom<WasmPreviewRequest> for RenderJsonRequest {
    type Error = String;

    fn try_from(value: WasmPreviewRequest) -> Result<Self, Self::Error> {
        let scope = match value.scope.as_str() {
            "current" => RenderJsonScope::Current,
            "flow" => RenderJsonScope::Flow,
            "step" => RenderJsonScope::Step {
                step_id: value
                    .step_id
                    .ok_or_else(|| "step scope requires step_id".to_string())?,
            },
            "widget" => RenderJsonScope::Widget {
                step_id: value
                    .step_id
                    .ok_or_else(|| "widget scope requires step_id".to_string())?,
                widget_id: value
                    .widget_id
                    .ok_or_else(|| "widget scope requires widget_id".to_string())?,
            },
            other => {
                return Err(format!(
                    "unsupported scope: {} (expected current|flow|step|widget)",
                    other
                ));
            }
        };

        let terminal_size = match (value.width, value.height) {
            (Some(width), Some(height)) => {
                Some(steply_core::terminal::TerminalSize { width, height })
            }
            (None, None) => None,
            _ => return Err("width and height must be provided together".to_string()),
        };

        Ok(RenderJsonRequest {
            scope,
            active_step_id: value.active_step_id,
            terminal_size,
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_exports {
    use super::*;
    use steply_core::preview::render::render_json;
    use steply_core::state::app::AppState;
    use steply_core::ui::renderer::Renderer;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn preview_render_json(yaml: &str, request_json: &str) -> Result<String, JsValue> {
        let request: WasmPreviewRequest =
            serde_json::from_str(request_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let request: RenderJsonRequest = request.try_into().map_err(|e| JsValue::from_str(&e))?;

        let loaded = steply_core::config::load_from_yaml_str(yaml)
            .map_err(|e| JsValue::from_str(e.as_str()))?;
        let mut state =
            AppState::with_tasks(loaded.flow, loaded.task_specs, loaded.task_subscriptions);
        let mut renderer = Renderer::default();
        let doc = render_json(
            &mut state,
            &request,
            &mut renderer,
            steply_core::terminal::TerminalSize {
                width: 100,
                height: 40,
            },
        )
        .map_err(|e| JsValue::from_str(e.as_str()))?;
        serde_json::to_string(&doc).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn parse_preview_request_json(input: &str) -> Result<String, JsValue> {
        let req: WasmPreviewRequest =
            serde_json::from_str(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let parsed: RenderJsonRequest = req.try_into().map_err(|e| JsValue::from_str(&e))?;
        serde_json::to_string(&parsed).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
