use crate::config::{FlashMode, NotifyMode};
use crate::state::{Activity, HookPayload, SessionInfo, State};
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

pub fn handle_hook_event(state: &mut State, payload: HookPayload) {
    // Capture env info for use in notifications
    if let Some(ref name) = payload.zellij_session {
        state.zellij_session_name = Some(name.clone());
    }
    if let Some(ref tp) = payload.term_program {
        state.term_program = Some(tp.clone());
    }

    let event = payload.hook_event.as_str();

    // SessionEnd → remove session
    if event == "SessionEnd" {
        state.sessions.remove(&payload.pane_id);
        return;
    }

    let activity = match event {
        "SessionStart" => Activity::Init,
        "PreToolUse" => {
            Activity::Tool(payload.tool_name.clone().unwrap_or_default())
        }
        "PostToolUse" | "PostToolUseFailure" => Activity::Thinking,
        "UserPromptSubmit" => Activity::Thinking,
        "PermissionRequest" => Activity::Waiting,
        "Notification" => {
            if let Some(session) = state.sessions.get_mut(&payload.pane_id) {
                session.last_event_ts = crate::state::unix_now();
            }
            return;
        }
        "Stop" => Activity::Done,
        "SubagentStop" => Activity::AgentDone,
        _ => Activity::Idle,
    };

    let (tab_index, tab_name) = state
        .pane_to_tab
        .get(&payload.pane_id)
        .cloned()
        .unzip();

    let session = state
        .sessions
        .entry(payload.pane_id)
        .or_insert_with(|| SessionInfo {
            session_id: payload.session_id.clone().unwrap_or_default(),
            pane_id: payload.pane_id,
            activity: Activity::Init,
            tab_name: None,
            tab_index: None,
            last_event_ts: 0,
            cwd: None,
        });

    if matches!(activity, Activity::Waiting) {
        match state.config.flash {
            FlashMode::Brief => {
                state.flash_deadlines.insert(
                    payload.pane_id,
                    crate::state::unix_now_ms() + crate::state::FLASH_DURATION_MS,
                );
            }
            FlashMode::Persist => {
                state.flash_deadlines.insert(payload.pane_id, u64::MAX);
            }
            FlashMode::Off => {}
        }

        // Desktop notification via terminal-notifier
        let should_notify = match state.config.notifications {
            NotifyMode::Always => true,
            NotifyMode::Unfocused => {
                // Notify only if the waiting session's tab is not the active tab
                let session_tab = state
                    .pane_to_tab
                    .get(&payload.pane_id)
                    .map(|(idx, _)| *idx);
                session_tab != state.active_tab_index
            }
            NotifyMode::Off => false,
        };
        if should_notify {
            let tab_label = state
                .pane_to_tab
                .get(&payload.pane_id)
                .map(|(_, name)| name.as_str())
                .unwrap_or("unknown");
            let title = "Claude Code - Permission Request";
            let message = format!("Waiting for permission in tab: {}", tab_label);
            let mut cmd_args = vec![
                "terminal-notifier".to_string(),
                "-title".to_string(),
                title.to_string(),
                "-message".to_string(),
                message,
                "-sender".to_string(),
                "dev.zellij.zellij".to_string(),
            ];
            // Click notification to pipe focus event
            if let Some(ref zj_session) = state.zellij_session_name {
                let focus_cmd = format!(
                    "zellij --session {} pipe --name zjbar:focus -- {}",
                    zj_session, payload.pane_id
                );
                cmd_args.push("-execute".to_string());
                cmd_args.push(focus_cmd);
            }
            let args: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();
            run_command(&args, BTreeMap::new());
        }
    } else {
        state.flash_deadlines.remove(&payload.pane_id);
    }

    session.activity = activity;
    session.last_event_ts = crate::state::unix_now();
    if let Some(sid) = &payload.session_id {
        session.session_id = sid.clone();
    }
    if let Some(cwd) = payload.cwd {
        session.cwd = Some(cwd);
    }
    if let Some((idx, name)) = tab_index.zip(tab_name) {
        session.tab_index = Some(idx);
        session.tab_name = Some(name);
    }
}
