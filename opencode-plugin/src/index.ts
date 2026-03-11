import type { Plugin } from "@opencode-ai/plugin";

const TOOL_MAP: Record<string, string> = {
  bash: "Bash",
  read: "Read",
  edit: "Edit",
  write: "Write",
  grep: "Grep",
  glob: "Glob",
  webfetch: "WebFetch",
  websearch: "WebSearch",
  todoreplace: "Task",
  todowrite: "Task",
};

function capitalize(s: string): string {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}

export const ZjbarPlugin: Plugin = async ({ $, directory }) => {
  const zellijSession = process.env.ZELLIJ_SESSION_NAME;
  const paneId = process.env.ZELLIJ_PANE_ID;
  // Exit silently if not running inside Zellij
  if (!zellijSession || !paneId) return {};

  const sessionId = crypto.randomUUID();
  const termProgram = process.env.TERM_PROGRAM || null;

  async function sendToZjbar(
    hookEvent: string,
    toolName?: string | null,
  ): Promise<void> {
    const payload = JSON.stringify({
      source: "opencode",
      pane_id: parseInt(paneId!, 10),
      session_id: sessionId,
      hook_event: hookEvent,
      tool_name: toolName || null,
      cwd: directory || null,
      zellij_session: zellijSession,
      term_program: termProgram,
    });
    try {
      await $`zellij -s ${zellijSession} pipe --name zjbar -- ${payload}`.quiet();
    } catch {
      // Silently ignore pipe failures
    }
  }

  // Send SessionStart on plugin load
  await sendToZjbar("SessionStart");

  return {
    event: async ({ event }) => {
      switch (event.type) {
        case "session.created":
          await sendToZjbar("SessionStart");
          break;
        case "session.idle":
          await sendToZjbar("Stop");
          break;
        case "session.deleted":
          await sendToZjbar("SessionEnd");
          break;
        case "permission.asked":
          await sendToZjbar("PermissionRequest");
          break;
        case "message.updated":
          await sendToZjbar("PostToolUse");
          break;
      }
    },

    "tool.execute.before": async (input) => {
      const toolName = TOOL_MAP[input.tool] || capitalize(input.tool);
      await sendToZjbar("PreToolUse", toolName);
    },

    "tool.execute.after": async () => {
      await sendToZjbar("PostToolUse");
    },
  };
};

export default ZjbarPlugin;
