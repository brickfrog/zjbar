import type { Plugin } from "@opencode-ai/plugin";
import type { Part, Permission } from "@opencode-ai/sdk";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { homedir, platform } from "node:os";
import { fileURLToPath } from "node:url";
import { spawn, execFile as execFileCb } from "node:child_process";

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

// -- Notification support --

interface ZjbarSettings {
  notifications?: "always" | "unfocused" | "off";
  notify_events?: string[];
}

const DEFAULT_NOTIFY_EVENTS = ["PermissionRequest", "Notification", "Stop"];
const NOTIFY_RATE_LIMIT_SECS = 10;
const lastNotifyTime: Record<string, number> = {};

function loadSettings(): ZjbarSettings {
  const settingsPath = join(
    homedir(),
    ".config",
    "zellij",
    "plugins",
    "zjbar.json",
  );
  try {
    if (existsSync(settingsPath)) {
      return JSON.parse(readFileSync(settingsPath, "utf-8"));
    }
  } catch {}
  return {};
}

function shouldNotify(
  hookEvent: string,
  paneId: string,
  termProgram: string | null,
): boolean {
  const settings = loadSettings();
  const mode = settings.notifications ?? "always";
  if (mode === "off") return false;

  const events = settings.notify_events ?? DEFAULT_NOTIFY_EVENTS;
  if (!events.includes(hookEvent)) return false;

  // Rate limit per pane
  const now = Math.floor(Date.now() / 1000);
  const key = `pane-${paneId}`;
  if (now - (lastNotifyTime[key] ?? 0) < NOTIFY_RATE_LIMIT_SECS) return false;
  lastNotifyTime[key] = now;

  if (mode === "unfocused" && platform() === "darwin" && termProgram) {
    try {
      const { execSync } = require("node:child_process");
      const frontApp = execSync(
        'osascript -e \'tell application "System Events" to get name of first application process whose frontmost is true\'',
        { encoding: "utf-8" },
      ).trim();
      let expected = termProgram;
      if (expected === "Apple_Terminal") expected = "Terminal";
      if (expected === "iTerm.app") expected = "iTerm2";
      if (frontApp === expected) return false;
    } catch {}
  }

  return true;
}

function sendNotification(
  hookEvent: string,
  paneId: string,
  zellijSession: string,
  termProgram: string | null,
  summary?: string | null,
): void {
  const appName = "OpenCode";
  const iconFile = "opencode-logo.png";
  const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
  const iconPath = join(pkgRoot, "assets", iconFile);

  let title: string;
  let message: string;

  switch (hookEvent) {
    case "PermissionRequest":
      title = `⚠ ${appName}`;
      message = summary || "Permission requested";
      break;
    case "Stop":
      title = `✅ ${appName}`;
      message = summary || "Task completed";
      break;
    case "Notification":
      title = appName;
      message = summary || "Notification received";
      break;
    default:
      title = appName;
      message = `Event: ${hookEvent}`;
      break;
  }

  if (platform() === "darwin") {
    if (hookEvent === "PermissionRequest") {
      process.stdout.write("\x07");
    }

    const focusCmd = termProgram
      ? `open -a '${termProgram}' && zellij -s '${zellijSession}' pipe --name zjbar:focus -- ${paneId}`
      : `zellij -s '${zellijSession}' pipe --name zjbar:focus -- ${paneId}`;

    const args = ["-title", title, "-message", message, "-execute", focusCmd];
    if (existsSync(iconPath)) {
      args.unshift("-contentImage", iconPath);
    }
    execFileCb("terminal-notifier", args, (err) => {
      if (err) {
        execFileCb("osascript", [
          "-e",
          `display notification "${message.replace(/"/g, '\\"')}" with title "${title.replace(/"/g, '\\"')}"`,
        ]);
      }
    });
  } else if (platform() === "linux") {
    execFileCb("notify-send", [title, message]);
  }
}

// -- Session summary extraction --

/** Strip markdown formatting and truncate text to maxLen at word boundary. */
function cleanAndTruncate(text: string, maxLen = 120): string {
  let cleaned = text
    .replace(/\*\*/g, "")
    .replace(/\*/g, "")
    .replace(/`/g, "")
    .replace(/^#+ /gm, "")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\n/g, " ")
    .replace(/ {2,}/g, " ")
    .trim();
  if (cleaned.length > maxLen) {
    cleaned = cleaned.slice(0, maxLen - 3);
    const lastSpace = cleaned.lastIndexOf(" ");
    if (lastSpace > 0) cleaned = cleaned.slice(0, lastSpace);
    cleaned += "...";
  }
  return cleaned;
}

/** Fetch the last assistant text from the session via SDK client. */
async function getSessionSummary(
  client: any,
  sessionId: string,
): Promise<string | null> {
  const res = await client.session.messages({
    path: { id: sessionId },
  });
  if (!res.data) return null;
  // Response is Array<{ info: Message, parts: Part[] }>
  const messages: any[] = Array.isArray(res.data) ? res.data : [];
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    if (msg.info?.role !== "assistant") continue;
    if (!msg.parts) continue;
    // Find last text part
    for (let j = msg.parts.length - 1; j >= 0; j--) {
      const part: Part = msg.parts[j];
      if (part.type === "text" && part.text) {
        return cleanAndTruncate(part.text);
      }
    }
  }
  return null;
}

// -- Singleton guard with local-priority --
//
// OpenCode may load the plugin from multiple sources (project-local
// ./opencode-plugin via opencode.json AND global npm cache). Both
// instances return event handler objects to OpenCode independently —
// we cannot "unregister" an already-returned handler. Instead we use a
// shared mutable flag: whichever instance does NOT own the flag makes
// its callbacks no-ops. A local (project-dir) instance always wins,
// even if the global one loaded first.

/** Determine if this module is loaded from a project-local path. */
function isLocalPlugin(): boolean {
  try {
    const thisDir = dirname(fileURLToPath(import.meta.url));
    // Local plugin lives under the project's opencode-plugin/ dir,
    // NOT inside node_modules.
    return !thisDir.includes("node_modules");
  } catch {
    return false;
  }
}

// Global mutable ref shared across all instances in the same process.
// The "active" instance sets this to its own id; other instances check
// before every callback and bail if they are not the active one.
const GUARD_KEY = "ZJBAR_OPENCODE_ACTIVE";

function acquireGuard(instanceId: string, local: boolean): boolean {
  const current = process.env[GUARD_KEY];
  if (!current) {
    // No one registered yet — claim it.
    process.env[GUARD_KEY] = instanceId;
    return true;
  }
  if (local && !current.startsWith("local:")) {
    // A global instance registered first, but we are local — take over.
    process.env[GUARD_KEY] = instanceId;
    return true;
  }
  // Someone else owns it and we can't preempt.
  return false;
}

function isActiveInstance(instanceId: string): boolean {
  return process.env[GUARD_KEY] === instanceId;
}

// -- Plugin entry --

export const ZjbarPlugin: Plugin = async ({ directory, client }) => {
  const zellijSession = process.env.ZELLIJ_SESSION_NAME;
  const paneId = process.env.ZELLIJ_PANE_ID;
  // Exit silently if not running inside Zellij
  if (!zellijSession || !paneId) return {};

  const local = isLocalPlugin();
  const instanceId = `${local ? "local" : "global"}:${crypto.randomUUID()}`;

  if (!acquireGuard(instanceId, local)) {
    // Another instance already owns the guard and we can't preempt.
    return {};
  }

  const sessionId = crypto.randomUUID();
  const termProgram = process.env.TERM_PROGRAM || null;
  let activeSessionId: string | null = null;

  // Debounced Stop notification: OpenCode cycles through busy→idle
  // multiple times during multi-step tasks (e.g. tool calls). We only
  // want to send a desktop notification when the session is truly done,
  // not on intermediate idle states. Wait STOP_DEBOUNCE_MS after the
  // last session.idle before sending the notification.
  const STOP_DEBOUNCE_MS = 2000;
  let stopDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Resolve zellij binary path once at startup
  let zellijBin = "zellij";
  try {
    const { execFileSync } = require("node:child_process");
    zellijBin = execFileSync("which", ["zellij"], { encoding: "utf-8" }).trim() || "zellij";
  } catch {
    // Check common paths
    for (const p of ["/opt/homebrew/bin/zellij", "/usr/local/bin/zellij", "/usr/bin/zellij"]) {
      if (existsSync(p)) { zellijBin = p; break; }
    }
  }

  function sendToZjbar(
    hookEvent: string,
    toolName?: string | null,
    summary?: string | null,
    skipDesktop?: boolean,
  ): void {
    // If a local instance took over after us, become a no-op.
    if (!isActiveInstance(instanceId)) return;

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
    // Fire-and-forget: zellij pipe blocks indefinitely, so we spawn detached
    // Use resolved absolute path to avoid PATH issues in spawned child
    const child = spawn(zellijBin, [
      "-s",
      zellijSession!,
      "pipe",
      "--name",
      "zjbar",
      "--",
      payload,
    ], { detached: true, stdio: "ignore" });
    child.unref();

    // Desktop notifications for key events
    if (!skipDesktop && shouldNotify(hookEvent, paneId!, termProgram)) {
      sendNotification(hookEvent, paneId!, zellijSession!, termProgram, summary);
    }
  }

  // Send SessionStart on plugin load
  sendToZjbar("SessionStart");

  return {
    event: async ({ event }) => {
      const ev = event as any;
      const eventType: string = ev.type;
      switch (eventType) {
        case "session.created":
          activeSessionId = ev.properties?.info?.id || null;
          sendToZjbar("SessionStart");
          break;
        case "session.status": {
          const status = ev.properties?.status?.type;
          if (status === "busy") {
            // Cancel any pending Stop notification — session is active again
            if (stopDebounceTimer) {
              clearTimeout(stopDebounceTimer);
              stopDebounceTimer = null;
            }
            // Send UserPromptSubmit to trigger Thinking (●) icon
            sendToZjbar("UserPromptSubmit");
          }
          break;
        }
        case "session.idle": {
          // Always send Stop to zjbar plugin to update tab state (✅ Done),
          // but skip automatic desktop notification here
          sendToZjbar("Stop", null, null, /* skipDesktop */ true);
          // Debounce: schedule desktop notification after a delay.
          // If session becomes busy again before the timer fires,
          // the timer is cancelled in the session.status(busy) handler.
          if (stopDebounceTimer) clearTimeout(stopDebounceTimer);
          const sid = ev.properties?.sessionID || activeSessionId;
          stopDebounceTimer = setTimeout(async () => {
            stopDebounceTimer = null;
            if (sid && client) {
              try {
                const summary = await getSessionSummary(client, sid);
                if (summary && shouldNotify("Stop", paneId!, termProgram)) {
                  sendNotification("Stop", paneId!, zellijSession!, termProgram, summary);
                }
              } catch {}
            }
          }, STOP_DEBOUNCE_MS);
          break;
        }
        case "session.deleted":
          sendToZjbar("SessionEnd");
          break;
        case "permission.asked": {
          const perm = ev.properties as Permission | undefined;
          const permTitle = perm?.title || null;
          sendToZjbar("PermissionRequest", null, permTitle);
          break;
        }
        case "message.updated": {
          // OpenCode uses message.updated (not message.created).
          // Track active session ID from user messages.
          const msgInfo = ev.properties?.info;
          if (msgInfo?.role === "user" && msgInfo?.sessionID) {
            activeSessionId = msgInfo.sessionID;
          }
          break;
        }
      }
    },

    "tool.execute.before": async (input) => {
      const toolName = TOOL_MAP[input.tool] || capitalize(input.tool);
      sendToZjbar("PreToolUse", toolName);
    },

    // NOTE: We intentionally omit "tool.execute.after" (PostToolUse).
    // OpenCode runs in-process, so before/after fire within milliseconds.
    // Sending PostToolUse would immediately overwrite the Tool icon with
    // Thinking (●) before Zellij can render it. The Tool state naturally
    // transitions on the next PreToolUse or session.idle (Stop).
  };
};

export default ZjbarPlugin;
