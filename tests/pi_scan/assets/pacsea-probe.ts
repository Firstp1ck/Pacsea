import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

const TOOL_NAMES = [
  "pacsea_scan_find",
  "pacsea_scan_grep",
  "pacsea_scan_ls",
  "pacsea_scan_read",
] as const;

/** Register inert scanner-shaped tools and a no-model introspection command. */
export default function pacseaProbe(pi: ExtensionAPI) {
  for (const name of TOOL_NAMES) {
    pi.registerTool({
      name,
      label: name,
      description: "Wave 0 inert capability-probe tool",
      parameters: Type.Object({}),
      async execute() {
        return {
          content: [{ type: "text" as const, text: "probe-only" }],
          details: {},
        };
      },
    });
  }

  pi.registerCommand("pacsea-probe-tools", {
    description: "Report the exact active tool allowlist without invoking a model",
    handler: async (_args, ctx) => {
      const active = [...pi.getActiveTools()].sort();
      ctx.ui.notify(`PACSEA_ACTIVE_TOOLS:${JSON.stringify(active)}`, "info");
      ctx.shutdown();
    },
  });
}
