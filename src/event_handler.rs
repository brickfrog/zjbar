use crate::state::{Activity, FlashMode, HookPayload, SessionInfo, State};

pub fn handle_hook_event(state: &mut State, payload: HookPayload) -> bool {
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
        return state.sessions.remove(&payload.pane_id).is_some();
    }

    let activity = match event {
        "SessionStart" => Activity::Init,
        "PreToolUse" => {
            let name = payload.tool_name.clone().unwrap_or_else(|| {
                eprintln!("[zjbar] PreToolUse event missing tool_name, defaulting to empty");
                String::new()
            });
            Activity::Tool(name)
        }
        "PostToolUse" | "PostToolUseFailure" => Activity::Thinking,
        "UserPromptSubmit" => Activity::Thinking,
        "PermissionRequest" => Activity::Waiting,
        "Notification" => Activity::Notification,
        "Stop" => Activity::Done,
        _ => return false, // Unknown events: preserve existing activity state
    };

    let (tab_index, tab_name) = state.pane_to_tab.get(&payload.pane_id).cloned().unzip();

    let session = state
        .sessions
        .entry(payload.pane_id)
        .or_insert_with(|| SessionInfo {
            session_id: payload.session_id.clone().unwrap_or_else(|| {
                eprintln!(
                    "[zjbar] hook event missing session_id for pane {}",
                    payload.pane_id
                );
                String::new()
            }),
            pane_id: payload.pane_id,
            activity: Activity::Init,
            tab_name: None,
            tab_index: None,
            last_event_ts: 0,
            cwd: None,
        });

    let mut flash_changed = false;
    if matches!(activity, Activity::Waiting | Activity::Notification) {
        match state.settings.flash {
            FlashMode::Brief => {
                let deadline = crate::state::unix_now_ms() + crate::state::FLASH_DURATION_MS;
                flash_changed =
                    state.flash_deadlines.insert(payload.pane_id, deadline) != Some(deadline);
            }
            FlashMode::Persist => {
                flash_changed =
                    state.flash_deadlines.insert(payload.pane_id, u64::MAX) != Some(u64::MAX);
            }
            FlashMode::Off => {}
        }
    } else {
        flash_changed = state.flash_deadlines.remove(&payload.pane_id).is_some();
    }

    // Validate state transition (permissive: log but apply)
    if !session.activity.can_transition_to(&activity) {
        eprintln!(
            "[zjbar] unexpected state transition: {:?} -> {:?} (pane {}, event {})",
            session.activity, activity, payload.pane_id, payload.hook_event
        );
    }
    let mut changed = session.activity != activity;
    session.activity = activity;
    let now = crate::state::unix_now();
    changed |= session.last_event_ts != now;
    session.last_event_ts = now;
    if let Some(sid) = &payload.session_id {
        changed |= session.session_id != *sid;
        session.session_id = sid.clone();
    }
    if let Some(cwd) = payload.cwd {
        changed |= session.cwd.as_ref() != Some(&cwd);
        session.cwd = Some(cwd);
    }
    if let Some((idx, name)) = tab_index.zip(tab_name) {
        changed |= session.tab_index != Some(idx) || session.tab_name.as_ref() != Some(&name);
        session.tab_index = Some(idx);
        session.tab_name = Some(name);
    }
    changed || flash_changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(event: &str, pane_id: u32) -> HookPayload {
        HookPayload {
            source: Some("claude".into()),
            session_id: Some("test-session".into()),
            pane_id,
            hook_event: event.into(),
            tool_name: None,
            cwd: None,
            zellij_session: None,
            term_program: None,
        }
    }

    fn make_tool_payload(tool: &str, pane_id: u32) -> HookPayload {
        let mut p = make_payload("PreToolUse", pane_id);
        p.tool_name = Some(tool.into());
        p
    }

    #[test]
    fn test_session_start_creates_session() {
        let mut state = State::default();
        let payload = make_payload("SessionStart", 1);
        handle_hook_event(&mut state, payload);

        let session = state.sessions.get(&1).expect("session should exist");
        assert_eq!(session.activity, Activity::Init);
        assert_eq!(session.pane_id, 1);
        assert_eq!(session.session_id, "test-session");
    }

    #[test]
    fn test_pre_tool_use_sets_tool_activity() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_tool_payload("Bash", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Tool("Bash".into()));
    }

    #[test]
    fn test_post_tool_use_sets_thinking() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_tool_payload("Bash", 1));
        handle_hook_event(&mut state, make_payload("PostToolUse", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Thinking);
    }

    #[test]
    fn test_post_tool_use_failure_sets_thinking() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_tool_payload("Bash", 1));
        handle_hook_event(&mut state, make_payload("PostToolUseFailure", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Thinking);
    }

    #[test]
    fn test_user_prompt_submit_sets_thinking() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_payload("UserPromptSubmit", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Thinking);
    }

    #[test]
    fn test_permission_request_sets_waiting_and_flash() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_payload("PermissionRequest", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Waiting);
        assert!(state.flash_deadlines.contains_key(&1));
    }

    #[test]
    fn test_notification_sets_notification_and_flash() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_payload("Notification", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Notification);
        assert!(state.flash_deadlines.contains_key(&1));
    }

    #[test]
    fn test_stop_sets_done_clears_flash() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        handle_hook_event(&mut state, make_payload("PermissionRequest", 1));
        assert!(state.flash_deadlines.contains_key(&1));

        handle_hook_event(&mut state, make_payload("Stop", 1));
        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Done);
        assert!(!state.flash_deadlines.contains_key(&1));
    }

    #[test]
    fn test_session_end_removes_session() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        assert!(state.sessions.get(&1).is_some());

        handle_hook_event(&mut state, make_payload("SessionEnd", 1));
        assert!(state.sessions.get(&1).is_none());
    }

    #[test]
    fn test_unknown_event_preserves_state() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        assert_eq!(state.sessions.get(&1).unwrap().activity, Activity::Init);

        handle_hook_event(&mut state, make_payload("UnknownEvent", 1));
        assert_eq!(state.sessions.get(&1).unwrap().activity, Activity::Init);
    }

    #[test]
    fn test_missing_tool_name_defaults_empty() {
        let mut state = State::default();
        handle_hook_event(&mut state, make_payload("SessionStart", 1));
        // PreToolUse with tool_name = None
        handle_hook_event(&mut state, make_payload("PreToolUse", 1));

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Tool("".into()));
    }

    #[test]
    fn test_zellij_session_captured() {
        let mut state = State::default();
        let mut payload = make_payload("SessionStart", 1);
        payload.zellij_session = Some("work".into());
        handle_hook_event(&mut state, payload);

        assert_eq!(state.zellij_session_name, Some("work".into()));
    }
}
