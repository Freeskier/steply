import { readFile } from "node:fs/promises";
import { renderFlowPreviewFromYaml } from "$lib/server/steply-preview";

const FLOW_YAML_URL = new URL(
  "../../../../tools/examples/flow_v1_all_widgets.yaml",
  import.meta.url,
);

export const load = async () => {
  const yaml = await readFile(FLOW_YAML_URL, "utf8");
  const request = {
    scope: "flow" as const,
    active_step_id: "inputs",
    width: 100,
    height: 40,
  };

  try {
    const rendered = renderFlowPreviewFromYaml(yaml, request);
    return {
      yaml,
      request,
      rendered,
      error: null,
    };
  } catch (error) {
    return {
      yaml,
      request,
      rendered: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};
