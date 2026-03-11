// zjbar-opencode-plugin.js — OpenCode plugin → zellij pipe bridge
// Translates OpenCode events into zjbar's unified HookPayload format.
//
// Install: cp this file to ~/.config/opencode/plugins/
// Or run: ./scripts/install-opencode.sh

const TOOL_MAP = {
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
}

function capitalize(s) {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s
}

export const ZjbarPlugin = async ({ $, directory }) => {
  const zellijSession = process.env.ZELLIJ_SESSION_NAME
  const paneId = process.env.ZELLIJ_PANE_ID
  // Exit silently if not running inside Zellij
  if (!zellijSession || !paneId) return {}

  const sessionId = crypto.randomUUID()
  const termProgram = process.env.TERM_PROGRAM || null

  async function sendToZjbar(hookEvent, toolName) {
    const payload = JSON.stringify({
      source: "opencode",
      pane_id: parseInt(paneId, 10),
      session_id: sessionId,
      hook_event: hookEvent,
      tool_name: toolName || null,
      cwd: directory || null,
      zellij_session: zellijSession,
      term_program: termProgram,
    })
    try {
      await $`zellij -s ${zellijSession} pipe --name zjbar -- ${payload}`.quiet()
    } catch {
      // Silently ignore pipe failures
    }
  }

  // Send SessionStart on plugin load
  await sendToZjbar("SessionStart")

  return {
    event: async ({ event }) => {
      switch (event.type) {
        case "session.created":
          await sendToZjbar("SessionStart")
          break
        case "session.idle":
          await sendToZjbar("Stop")
          break
        case "session.deleted":
          await sendToZjbar("SessionEnd")
          break
        case "permission.asked":
          await sendToZjbar("PermissionRequest")
          break
        case "message.updated":
          await sendToZjbar("PostToolUse")
          break
      }
    },

    "tool.execute.before": async (input) => {
      const toolName = TOOL_MAP[input.tool] || capitalize(input.tool)
      await sendToZjbar("PreToolUse", toolName)
    },

    "tool.execute.after": async () => {
      await sendToZjbar("PostToolUse")
    },
  }
}
