import { createRequire } from "node:module";
var __require = /* @__PURE__ */ createRequire(import.meta.url);

// src/index.ts
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { homedir, platform } from "node:os";
import { spawn, execFile as execFileCb } from "node:child_process";
var TOOL_MAP = {
  bash: "Bash",
  read: "Read",
  edit: "Edit",
  write: "Write",
  grep: "Grep",
  glob: "Glob",
  webfetch: "WebFetch",
  websearch: "WebSearch",
  todoreplace: "Task",
  todowrite: "Task"
};
function capitalize(s) {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}
var DEFAULT_NOTIFY_EVENTS = ["PermissionRequest", "Notification", "Stop"];
var NOTIFY_RATE_LIMIT_SECS = 10;
var lastNotifyTime = {};
function loadSettings() {
  const settingsPath = join(homedir(), ".config", "zellij", "plugins", "zjbar.json");
  try {
    if (existsSync(settingsPath)) {
      return JSON.parse(readFileSync(settingsPath, "utf-8"));
    }
  } catch {}
  return {};
}
function shouldNotify(hookEvent, paneId, termProgram) {
  const settings = loadSettings();
  const mode = settings.notifications ?? "always";
  if (mode === "off")
    return false;
  const events = settings.notify_events ?? DEFAULT_NOTIFY_EVENTS;
  if (!events.includes(hookEvent))
    return false;
  const now = Math.floor(Date.now() / 1000);
  const key = `pane-${paneId}`;
  if (now - (lastNotifyTime[key] ?? 0) < NOTIFY_RATE_LIMIT_SECS)
    return false;
  lastNotifyTime[key] = now;
  if (mode === "unfocused" && platform() === "darwin" && termProgram) {
    try {
      const { execSync } = __require("node:child_process");
      const frontApp = execSync(`osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true'`, { encoding: "utf-8" }).trim();
      let expected = termProgram;
      if (expected === "Apple_Terminal")
        expected = "Terminal";
      if (expected === "iTerm.app")
        expected = "iTerm2";
      if (frontApp === expected)
        return false;
    } catch {}
  }
  return true;
}
function sendNotification(hookEvent, paneId, zellijSession, termProgram) {
  const appName = "OpenCode";
  const iconFile = "opencode-logo.png";
  const pluginDir = join(homedir(), ".config", "zellij", "plugins");
  const iconPath = join(pluginDir, iconFile);
  let title;
  let message;
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
    const focusCmd = termProgram ? `open -a '${termProgram}' && zellij -s '${zellijSession}' pipe --name zjbar:focus -- ${paneId}` : `zellij -s '${zellijSession}' pipe --name zjbar:focus -- ${paneId}`;
    const args = ["-title", title, "-message", message, "-execute", focusCmd];
    if (existsSync(iconPath)) {
      args.unshift("-contentImage", iconPath);
    }
    execFileCb("terminal-notifier", args, (err) => {
      if (err) {
        execFileCb("osascript", [
          "-e",
          `display notification "${message.replace(/"/g, "\\\"")}" with title "${title.replace(/"/g, "\\\"")}"`
        ]);
      }
    });
  } else if (platform() === "linux") {
    execFileCb("notify-send", [title, message]);
  }
}
var ZjbarPlugin = async ({ directory }) => {
  const zellijSession = process.env.ZELLIJ_SESSION_NAME;
  const paneId = process.env.ZELLIJ_PANE_ID;
  if (!zellijSession || !paneId)
    return {};
  const sessionId = crypto.randomUUID();
  const termProgram = process.env.TERM_PROGRAM || null;
  let zellijBin = "zellij";
  try {
    const { execFileSync } = __require("node:child_process");
    zellijBin = execFileSync("which", ["zellij"], { encoding: "utf-8" }).trim() || "zellij";
  } catch {
    for (const p of ["/opt/homebrew/bin/zellij", "/usr/local/bin/zellij", "/usr/bin/zellij"]) {
      if (existsSync(p)) {
        zellijBin = p;
        break;
      }
    }
  }
  function sendToZjbar(hookEvent, toolName) {
    const payload = JSON.stringify({
      source: "opencode",
      pane_id: parseInt(paneId, 10),
      session_id: sessionId,
      hook_event: hookEvent,
      tool_name: toolName || null,
      cwd: directory || null,
      zellij_session: zellijSession,
      term_program: termProgram
    });
    const child = spawn(zellijBin, [
      "-s",
      zellijSession,
      "pipe",
      "--name",
      "zjbar",
      "--",
      payload
    ], { detached: true, stdio: "ignore" });
    child.unref();
    if (shouldNotify(hookEvent, paneId, termProgram)) {
      sendNotification(hookEvent, paneId, zellijSession, termProgram);
    }
  }
  sendToZjbar("SessionStart");
  return {
    event: async ({ event }) => {
      const ev = event;
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
    }
  };
};
var src_default = ZjbarPlugin;
export {
  src_default as default,
  ZjbarPlugin
};
