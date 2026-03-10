let initialized = false;
let wasmModule: {
  default: (input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module) => Promise<unknown>;
  preview_config_docs_json: () => string;
} | null = null;

async function loadModule() {
  if (!wasmModule) {
    wasmModule = await import('$lib/steply-wasm/pkg/steply_wasm.js');
  }
  return wasmModule;
}

export async function initWasmPreview() {
  if (initialized) return;
  const mod = await loadModule();
  await mod.default();
  initialized = true;
}

export async function previewDocsJson() {
  await initWasmPreview();
  return loadModule().then((mod) => mod.preview_config_docs_json());
}
