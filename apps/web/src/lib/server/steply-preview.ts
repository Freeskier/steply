import { createRequire } from 'node:module';

export type PreviewScope = 'current' | 'flow' | 'step' | 'widget';

export type PreviewRequest = {
  scope: PreviewScope;
  step_id?: string;
  widget_id?: string;
  active_step_id?: string;
  width?: number;
  height?: number;
};

type SteplyWasmExports = {
  parse_preview_request_json(input: string): string;
  preview_render_json(yaml: string, request_json: string): string;
};

let wasmModule: SteplyWasmExports | null = null;

function getWasmModule(): SteplyWasmExports {
  if (wasmModule) return wasmModule;
  const require = createRequire(import.meta.url);
  wasmModule = require('../steply-wasm/pkg/steply_wasm.js') as SteplyWasmExports;
  return wasmModule;
}

export function renderFlowPreviewFromYaml(yaml: string, request: PreviewRequest): unknown {
  const wasm = getWasmModule();
  const requestJson = JSON.stringify(request);
  const rendered = wasm.preview_render_json(yaml, requestJson);
  return JSON.parse(rendered);
}
