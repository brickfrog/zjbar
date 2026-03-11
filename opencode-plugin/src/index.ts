import type { Plugin } from "@opencode-ai/plugin";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { homedir, platform } from "node:os";
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
): void {
  const appName = "OpenCode";
  const iconFile = "opencode-logo.png";
  const pluginDir = join(homedir(), ".config", "zellij", "plugins");
  const iconPath = join(pluginDir, iconFile);

  let title: string;
  let message: string;

  switch (hookEvent) {
    case "PermissionRequest":
      title = `⚠ ${appName}`;
      message = "Permission requested";
      break;
    case "Stop":
      title = `✅ ${appName}`;
      message = "Task completed";
      break;
    case "Notification":
      title = appName;
      message = "Notification received";
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

// -- Plugin entry --

export const ZjbarPlugin: Plugin = async ({ directory }) => {
  const zellijSession = process.env.ZELLIJ_SESSION_NAME;
  const paneId = process.env.ZELLIJ_PANE_ID;
  // Exit silently if not running inside Zellij
  if (!zellijSession || !paneId) return {};

  const sessionId = crypto.randomUUID();
  const termProgram = process.env.TERM_PROGRAM || null;

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
  ): void {
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
    if (shouldNotify(hookEvent, paneId!, termProgram)) {
      sendNotification(hookEvent, paneId!, zellijSession!, termProgram);
    }
  }

  // Send SessionStart on plugin load
  sendToZjbar("SessionStart");

  return {
    event: async ({ event }) => {
      const ev = event as any;
      switch (ev.type) {
        case "session.created":
          sendToZjbar("SessionStart");
          break;
        case "session.idle":
          sendToZjbar("Stop");
          break;
        case "session.deleted":
          sendToZjbar("SessionEnd");
          break;
        case "permission.asked":
          sendToZjbar("PermissionRequest");
          break;
        case "message.created":
          sendToZjbar("UserPromptSubmit");
          break;
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
