use serde::{Deserialize, Serialize};
use steply_core::preview::RenderJsonRequest;
use steply_core::terminal::{KeyCode, KeyEvent, KeyModifiers};

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
        RenderJsonRequest::from_named_parts(
            Some(value.scope),
            value.step_id,
            value.widget_id,
            value.active_step_id,
            value.width,
            value.height,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmKeyEvent {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl TryFrom<WasmKeyEvent> for KeyEvent {
    type Error = String;

    fn try_from(value: WasmKeyEvent) -> Result<Self, Self::Error> {
        let code = if value.key == "Tab" && value.shift {
            KeyCode::BackTab
        } else {
            map_key_code(value.key.as_str())?
        };
        let mut modifiers = KeyModifiers::NONE;
        if value.ctrl {
            modifiers = modifiers.union(KeyModifiers::CONTROL);
        }
        if value.alt {
            modifiers = modifiers.union(KeyModifiers::ALT);
        }
        if value.shift {
            modifiers = modifiers.union(KeyModifiers::SHIFT);
        }
        Ok(KeyEvent { code, modifiers })
    }
}

fn map_key_code(key: &str) -> Result<KeyCode, String> {
    let code = match key {
        "Enter" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Esc,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Unidentified" => KeyCode::Unknown,
        _ => {
            if key.chars().count() == 1 {
                KeyCode::Char(key.chars().next().expect("single-char key"))
            } else {
                KeyCode::Unknown
            }
        }
    };
    Ok(code)
}

#[cfg(target_arch = "wasm32")]
mod wasm_exports {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use steply_core::preview::{PreviewService, PreviewServiceOptions};
    use steply_core::runtime::effect::Effect;
    use steply_core::runtime::intent::Intent;
    use steply_core::runtime::key_bindings::KeyBindings;
    use steply_core::runtime::reducer::Reducer;
    use steply_core::ui::renderer::{Renderer, RendererConfig};
    use wasm_bindgen::prelude::*;

    struct PreviewSession {
        state: steply_core::state::app::AppState,
        renderer: Renderer,
        key_bindings: KeyBindings,
    }

    thread_local! {
        static PREVIEW_SESSIONS: RefCell<HashMap<String, PreviewSession>> = RefCell::new(HashMap::new());
    }
    static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

    fn parse_request(request_json: &str) -> Result<RenderJsonRequest, JsValue> {
        let req: WasmPreviewRequest =
            serde_json::from_str(request_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        req.try_into()
            .map_err(|e: String| JsValue::from_str(e.as_str()))
    }

    fn with_session_mut<T>(
        session_id: &str,
        f: impl FnOnce(&mut PreviewSession) -> Result<T, JsValue>,
    ) -> Result<T, JsValue> {
        PREVIEW_SESSIONS.with(|cell| {
            let mut map = cell.borrow_mut();
            let Some(session) = map.get_mut(session_id) else {
                return Err(JsValue::from_str("unknown preview session id"));
            };
            f(session)
        })
    }

    fn render_session(
        session: &mut PreviewSession,
        request: &RenderJsonRequest,
    ) -> Result<String, JsValue> {
        let mut effective_request = request.clone();
        // Session mode keeps its own navigation/focus state.
        // Re-applying active_step_id on each render would reset interactive progress.
        effective_request.active_step_id = None;
        let doc = steply_core::preview::render::render_json(
            &mut session.state,
            &effective_request,
            &mut session.renderer,
            steply_core::terminal::TerminalSize {
                width: 100,
                height: 40,
            },
        )
        .map_err(|e| JsValue::from_str(e.as_str()))?;
        serde_json::to_string(&doc).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    fn apply_effects(session: &mut PreviewSession, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Action(action) => {
                    let _ = session.state.handle_action(action);
                }
                Effect::System(event) => {
                    let _ = session.state.handle_system_event(event);
                }
                Effect::RequestRender | Effect::Schedule(_) => {}
            }
        }
        // Web preview mode intentionally skips task execution/scheduler runtime.
        let _ = session.state.take_pending_task_invocations();
        let _ = session.state.take_pending_scheduler_commands();
    }

    #[wasm_bindgen]
    pub fn preview_validate_yaml(yaml: &str) -> Result<String, JsValue> {
        let loaded = steply_core::config::load_from_yaml_str(yaml)
            .map_err(|e| JsValue::from_str(e.as_str()))?;
        Ok(format!(
            "ok: steps={}, tasks={}, subscriptions={}",
            loaded.flow.steps().len(),
            loaded.task_specs.len(),
            loaded.task_subscriptions.len()
        ))
    }

    #[wasm_bindgen]
    pub fn preview_render_json(yaml: &str, request_json: &str) -> Result<String, JsValue> {
        let request = parse_request(request_json)?;

        let mut service =
            PreviewService::from_yaml_str_with_options(yaml, PreviewServiceOptions::default())
                .map_err(|e| JsValue::from_str(e.as_str()))?;
        let doc = service
            .render(&request)
            .map_err(|e| JsValue::from_str(e.as_str()))?;
        serde_json::to_string(&doc).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn parse_preview_request_json(input: &str) -> Result<String, JsValue> {
        let req: WasmPreviewRequest =
            serde_json::from_str(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let parsed: RenderJsonRequest = req
            .try_into()
            .map_err(|e: String| JsValue::from_str(e.as_str()))?;
        serde_json::to_string(&parsed).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn preview_session_create(yaml: &str) -> Result<String, JsValue> {
        let loaded = steply_core::config::load_from_yaml_str(yaml)
            .map_err(|e| JsValue::from_str(e.as_str()))?;
        let session = PreviewSession {
            state: loaded.into_app_state(),
            renderer: Renderer::new(RendererConfig {
                chrome_enabled: true,
            }),
            key_bindings: KeyBindings::new(),
        };
        let id = format!("ps_{}", NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed));
        PREVIEW_SESSIONS.with(|cell| {
            cell.borrow_mut().insert(id.clone(), session);
        });
        Ok(id)
    }

    #[wasm_bindgen]
    pub fn preview_session_render(session_id: &str, request_json: &str) -> Result<String, JsValue> {
        let request = parse_request(request_json)?;
        with_session_mut(session_id, |session| render_session(session, &request))
    }

    #[wasm_bindgen]
    pub fn preview_session_key_event(
        session_id: &str,
        key_event_json: &str,
        request_json: &str,
    ) -> Result<String, JsValue> {
        let key_event: WasmKeyEvent =
            serde_json::from_str(key_event_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let key_event: KeyEvent = key_event
            .try_into()
            .map_err(|e: String| JsValue::from_str(e.as_str()))?;
        let request = parse_request(request_json)?;

        with_session_mut(session_id, |session| {
            let intent = session
                .key_bindings
                .resolve(key_event)
                .unwrap_or(Intent::InputKey(key_event));
            let effects = Reducer::reduce(&mut session.state, intent);
            apply_effects(session, effects);
            render_session(session, &request)
        })
    }

    #[wasm_bindgen]
    pub fn preview_session_dispose(session_id: &str) -> bool {
        PREVIEW_SESSIONS.with(|cell| cell.borrow_mut().remove(session_id).is_some())
    }
}
