import { createRequire } from "node:module";

export type PreviewScope = "current" | "flow" | "step" | "widget";

export type PreviewRequest = {
  scope: PreviewScope;
  step_id?: string;
  widget_id?: string;
  active_step_id?: string;
  width?: number;
  height?: number;
};

export type PreviewKeyEvent = {
  key: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
};

type SteplyWasmExports = {
  parse_preview_request_json(input: string): string;
  preview_render_json(yaml: string, request_json: string): string;
  preview_session_create(yaml: string): string;
  preview_session_render(session_id: string, request_json: string): string;
  preview_session_key_event(
    session_id: string,
    key_event_json: string,
    request_json: string,
  ): string;
  preview_session_dispose(session_id: string): boolean;
};

let wasmModule: SteplyWasmExports | null = null;

function getWasmModule(): SteplyWasmExports {
  if (wasmModule) return wasmModule;
  const require = createRequire(import.meta.url);
  wasmModule =
    require("../steply-wasm/pkg/steply_wasm.js") as SteplyWasmExports;
  return wasmModule;
}

export function renderFlowPreviewFromYaml(
  yaml: string,
  request: PreviewRequest,
): unknown {
  const wasm = getWasmModule();
  const requestJson = JSON.stringify(request);
  const rendered = wasm.preview_render_json(yaml, requestJson);
  return JSON.parse(rendered);
}

export function createPreviewSession(yaml: string): string {
  return getWasmModule().preview_session_create(yaml);
}

export function renderPreviewSession(
  sessionId: string,
  request: PreviewRequest,
): unknown {
  const rendered = getWasmModule().preview_session_render(
    sessionId,
    JSON.stringify(request),
  );
  return JSON.parse(rendered);
}

export function sendPreviewSessionKey(
  sessionId: string,
  keyEvent: PreviewKeyEvent,
  request: PreviewRequest,
): unknown {
  const rendered = getWasmModule().preview_session_key_event(
    sessionId,
    JSON.stringify(keyEvent),
    JSON.stringify(request),
  );
  return JSON.parse(rendered);
}

export function disposePreviewSession(sessionId: string): boolean {
  return getWasmModule().preview_session_dispose(sessionId);
}
