mod choir_status;
mod config;
mod event_handler;
mod render;
mod state;
mod tab_pane_map;

/// Stub out WASM host imports so that `cargo test` can link on native targets.
/// These functions are provided by the Zellij runtime when running as a plugin;
/// for unit tests we just need them to exist so the linker is happy.
#[cfg(all(test, not(target_family = "wasm")))]
#[no_mangle]
extern "C" fn host_run_plugin_command() {}

use choir_status::{ChoirStatus, StatusBarError, StatusBarState, CHOIR_POLL_INTERVAL_MS};
use config::BarConfig;
use state::{
    unix_now, unix_now_ms, HookPayload, MenuAction, SessionInfo, SettingKey, Settings, State,
    ViewMode,
};
use std::collections::{BTreeMap, HashSet};
use zellij_tile::prelude::*;

const DONE_TIMEOUT: u64 = 30;
const TIMER_INTERVAL: f64 = 1.0;
const FLASH_TICK: f64 = 0.25;

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = BarConfig::from_kdl(&configuration);

        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::Timer,
            EventType::Mouse,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
        ]);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::RunCommands,
        ]);
        set_timeout(TIMER_INTERVAL);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                let new_active = tabs.iter().find(|t| t.active).map(|t| t.position);
                if new_active != self.active_tab_index {
                    if let Some(idx) = new_active {
                        self.clear_flashes_on_tab(idx);
                    }
                }
                self.active_tab_index = new_active;
                self.tabs = tabs;
                self.rebuild_pane_map();
                true
            }
            Event::PaneUpdate(manifest) => {
                self.pane_manifest = Some(manifest);
                self.rebuild_pane_map();
                true
            }
            Event::ModeUpdate(mode_info) => {
                self.input_mode = mode_info.mode;
                if let Some(name) = mode_info.session_name {
                    self.zellij_session_name = Some(name);
                }
                true
            }
            Event::Mouse(Mouse::LeftClick(row, col)) => {
                // Check prefix click → toggle settings menu
                if let Some((start, end)) = self.prefix_click_region {
                    if row == 0 && col >= start && col < end {
                        self.view_mode = match self.view_mode {
                            ViewMode::Normal => ViewMode::Settings,
                            ViewMode::Settings => ViewMode::Normal,
                        };
                        return true;
                    }
                }

                match self.view_mode {
                    ViewMode::Normal => {
                        for region in &self.click_regions {
                            if row == region.row && col >= region.start_col && col < region.end_col
                            {
                                if region.is_waiting {
                                    focus_terminal_pane(region.pane_id, false);
                                } else {
                                    switch_tab_to(region.tab_index as u32 + 1);
                                }
                                return false;
                            }
                        }
                        false
                    }
                    ViewMode::Settings => {
                        for region in &self.menu_click_regions {
                            if col >= region.start_col && col < region.end_col {
                                match &region.action {
                                    MenuAction::ToggleSetting(key) => {
                                        match key {
                                            SettingKey::Flash => {
                                                self.settings.flash = self.settings.flash.cycle();
                                            }
                                            SettingKey::ElapsedTime => {
                                                self.settings.elapsed_time =
                                                    !self.settings.elapsed_time;
                                            }
                                            SettingKey::Notifications => {
                                                self.settings.notifications =
                                                    self.settings.notifications.cycle();
                                            }
                                        }
                                        self.save_config();
                                    }
                                    MenuAction::CloseMenu => {
                                        self.view_mode = ViewMode::Normal;
                                    }
                                }
                                return true;
                            }
                        }
                        false
                    }
                }
            }
            Event::Timer(_) => {
                let stale_changed = self.cleanup_stale_sessions();
                let flash_changed = self.cleanup_expired_flashes();
                let poll_started = self.poll_choir_status_if_due();
                let has_flashes = self.has_active_flashes();
                if has_flashes {
                    set_timeout(FLASH_TICK);
                } else {
                    set_timeout(TIMER_INTERVAL);
                }
                has_flashes
                    || stale_changed
                    || flash_changed
                    || poll_started
                    || self.has_elapsed_display()
            }
            Event::RunCommandResult(exit_code, stdout, _stderr, context) => {
                match context.get("type").map(|s| s.as_str()) {
                    Some("choir_status_poll") => {
                        self.choir_poll_inflight = false;
                        if exit_code == Some(0) {
                            let raw = String::from_utf8_lossy(&stdout);
                            match choir_status::parse_status_bar_state_response(raw.trim()) {
                                Ok(snapshot) => self.apply_choir_snapshot(snapshot),
                                Err(StatusBarError::SchemaAhead(version)) => {
                                    self.apply_choir_status_error(ChoirStatus::SchemaAhead(
                                        version,
                                    ));
                                }
                                Err(e) => {
                                    eprintln!("[zjbar] failed to parse choir status: {e}");
                                    self.apply_choir_status_error(ChoirStatus::Invalid(
                                        e.to_string(),
                                    ));
                                }
                            }
                        } else {
                            self.apply_choir_status_error(ChoirStatus::NoChoir);
                        }
                    }
                    Some("load_config") if exit_code == Some(0) => {
                        let raw = String::from_utf8_lossy(&stdout);
                        match serde_json::from_str::<Settings>(raw.trim()) {
                            Ok(settings) => {
                                self.settings = settings;
                            }
                            Err(e) => {
                                eprintln!("[zjbar] failed to parse config file: {e}");
                            }
                        }
                        self.config_loaded = true;
                    }
                    _ => {}
                }
                true
            }
            Event::PermissionRequestResult(_) => {
                set_selectable(false);
                if !self.config_loaded {
                    self.load_config();
                }
                self.poll_choir_status_if_due();
                self.request_sync();
                false
            }
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        match pipe_message.name.as_str() {
            "zjbar" => {
                let payload_str = match pipe_message.payload {
                    Some(ref s) => s,
                    None => return false,
                };
                let payload: HookPayload = match serde_json::from_str(payload_str) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[zjbar] failed to parse hook payload: {e}");
                        return false;
                    }
                };
                if let Some(reason) = payload.validate() {
                    eprintln!("[zjbar] invalid hook payload: {reason}");
                    return false;
                }
                event_handler::handle_hook_event(self, payload);
                self.broadcast_sessions();
                true
            }
            "zjbar:focus" => {
                if let Some(ref payload) = pipe_message.payload {
                    if let Ok(pane_id) = payload.trim().parse::<u32>() {
                        focus_terminal_pane(pane_id, false);
                    }
                }
                false
            }
            "zjbar:request" => {
                self.broadcast_sessions();
                false
            }
            "zjbar:sync" => {
                if let Some(ref payload) = pipe_message.payload {
                    match serde_json::from_str::<BTreeMap<u32, SessionInfo>>(payload) {
                        Ok(sessions) => {
                            self.merge_sessions(sessions);
                            return true;
                        }
                        Err(e) => {
                            eprintln!("[zjbar] failed to parse sync payload: {e}");
                        }
                    }
                }
                false
            }
            "zjbar:settings" => {
                if let Some(ref payload) = pipe_message.payload {
                    match serde_json::from_str::<Settings>(payload) {
                        Ok(settings) => {
                            self.settings = settings;
                            self.config_loaded = true;
                            return true;
                        }
                        Err(e) => {
                            eprintln!("[zjbar] failed to parse settings payload: {e}");
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        render::render_status_bar(self, rows, cols);
    }
}

impl State {
    fn poll_choir_status_if_due(&mut self) -> bool {
        if self.choir_poll_inflight {
            return false;
        }
        let now = unix_now_ms();
        if self.choir_last_poll_ms != 0
            && now.saturating_sub(self.choir_last_poll_ms) < CHOIR_POLL_INTERVAL_MS
        {
            return false;
        }
        self.choir_last_poll_ms = now;
        self.choir_poll_inflight = true;
        let mut ctx = BTreeMap::new();
        ctx.insert("type".into(), "choir_status_poll".into());
        let cmd = choir_status::poll_command(&self.config.choir_socket);
        run_command(&["sh", "-c", &cmd], ctx);
        false
    }

    fn apply_choir_snapshot(&mut self, snapshot: StatusBarState) {
        let now = unix_now_ms();
        let next_attention: HashSet<u32> = snapshot
            .panes
            .iter()
            .filter(|pane| pane.attention_needed)
            .map(|pane| pane.zellij_pane_id)
            .collect();

        for pane_id in next_attention.difference(&self.attention_panes) {
            self.flash_deadlines
                .insert(*pane_id, now + state::FLASH_DURATION_MS);
        }
        for pane_id in self.attention_panes.difference(&next_attention) {
            self.flash_deadlines.remove(pane_id);
        }

        self.attention_panes = next_attention;
        self.choir_status = ChoirStatus::Ready(snapshot);
    }

    fn apply_choir_status_error(&mut self, status: ChoirStatus) {
        self.attention_panes.clear();
        self.flash_deadlines.clear();
        self.choir_status = status;
    }

    fn rebuild_pane_map(&mut self) {
        if let Some(ref manifest) = self.pane_manifest {
            self.pane_to_tab = tab_pane_map::build_pane_to_tab_map(&self.tabs, manifest);
            self.refresh_session_tab_names();
            self.remove_dead_panes();
        }
    }

    fn refresh_session_tab_names(&mut self) {
        for session in self.sessions.values_mut() {
            if let Some((idx, name)) = self.pane_to_tab.get(&session.pane_id) {
                session.tab_index = Some(*idx);
                session.tab_name = Some(name.clone());
            }
        }
    }

    fn remove_dead_panes(&mut self) {
        self.sessions
            .retain(|pane_id, _| self.pane_to_tab.contains_key(pane_id));
    }

    fn cleanup_stale_sessions(&mut self) -> bool {
        let now = unix_now();
        let mut changed = false;
        for (&pane_id, session) in self.sessions.iter_mut() {
            match session.activity {
                state::Activity::Done | state::Activity::AgentDone => {
                    if now.saturating_sub(session.last_event_ts) >= DONE_TIMEOUT {
                        if !session.activity.can_transition_to(&state::Activity::Idle) {
                            eprintln!(
                                "[zjbar] unexpected timeout transition: {:?} -> Idle (pane {})",
                                session.activity, pane_id
                            );
                        }
                        session.activity = state::Activity::Idle;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        changed
    }

    fn clear_flashes_on_tab(&mut self, tab_idx: usize) {
        let pane_ids: Vec<u32> = self
            .sessions
            .values()
            .filter(|s| s.tab_index == Some(tab_idx))
            .map(|s| s.pane_id)
            .collect();
        for pane_id in pane_ids {
            self.flash_deadlines.remove(&pane_id);
        }
    }

    fn has_active_flashes(&self) -> bool {
        let now = unix_now_ms();
        self.flash_deadlines
            .values()
            .any(|&deadline| now < deadline)
    }

    fn cleanup_expired_flashes(&mut self) -> bool {
        let before = self.flash_deadlines.len();
        let now = unix_now_ms();
        self.flash_deadlines.retain(|_, deadline| now < *deadline);
        self.flash_deadlines.len() != before
    }

    fn has_elapsed_display(&self) -> bool {
        if !self.settings.elapsed_time {
            return false;
        }
        let now = unix_now();
        self.sessions.values().any(|s| {
            !matches!(s.activity, state::Activity::Idle)
                && now.saturating_sub(s.last_event_ts) >= DONE_TIMEOUT
        })
    }

    fn request_sync(&self) {
        pipe_message_to_plugin(MessageToPlugin::new("zjbar:request"));
    }

    fn broadcast_sessions(&self) {
        let mut msg = MessageToPlugin::new("zjbar:sync");
        msg.message_payload = Some(match serde_json::to_string(&self.sessions) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("[zjbar] failed to serialize sessions for sync: {e}");
                String::from("{}")
            }
        });
        pipe_message_to_plugin(msg);
    }

    fn broadcast_settings(&self) {
        let mut msg = MessageToPlugin::new("zjbar:settings");
        msg.message_payload = Some(match serde_json::to_string(&self.settings) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("[zjbar] failed to serialize settings for broadcast: {e}");
                String::from("{}")
            }
        });
        pipe_message_to_plugin(msg);
    }

    fn merge_sessions(&mut self, incoming: BTreeMap<u32, SessionInfo>) {
        for (pane_id, mut session) in incoming {
            let dominated = self
                .sessions
                .get(&pane_id)
                .map(|existing| session.last_event_ts >= existing.last_event_ts)
                .unwrap_or(true);
            if dominated {
                if let Some((idx, name)) = self.pane_to_tab.get(&pane_id) {
                    session.tab_index = Some(*idx);
                    session.tab_name = Some(name.clone());
                }
                self.sessions.insert(pane_id, session);
            }
        }
    }

    fn load_config(&self) {
        let mut ctx = BTreeMap::new();
        ctx.insert("type".into(), "load_config".into());
        run_command(
            &[
                "sh",
                "-c",
                "cat \"$HOME/.config/zellij/plugins/zjbar.json\" 2>/dev/null || echo '{}'",
            ],
            ctx,
        );
    }

    fn save_config(&self) {
        if !self.config_loaded {
            return;
        }
        self.broadcast_settings();
        let json = match serde_json::to_string(&self.settings) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("[zjbar] failed to serialize settings for save: {e}");
                return;
            }
        };
        let json_esc = json.replace('\'', "'\\''");
        let cmd = format!(
            "mkdir -p \"$HOME/.config/zellij/plugins\" && printf '%s' '{}' > \"$HOME/.config/zellij/plugins/zjbar.json\"",
            json_esc
        );
        let mut ctx = BTreeMap::new();
        ctx.insert("type".into(), "save_config".into());
        run_command(&["sh", "-c", &cmd], ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::Activity;

    fn make_session(pane_id: u32, activity: Activity, ts: u64) -> SessionInfo {
        SessionInfo {
            session_id: format!("session-{pane_id}"),
            pane_id,
            activity,
            tab_name: None,
            tab_index: None,
            last_event_ts: ts,
            cwd: None,
        }
    }

    #[test]
    fn test_merge_sessions_new_entry() {
        let mut state = State::default();
        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));
        state.merge_sessions(incoming);

        let session = state.sessions.get(&1).expect("session should exist");
        assert_eq!(session.activity, Activity::Thinking);
        assert_eq!(session.last_event_ts, 100);
    }

    #[test]
    fn test_merge_sessions_newer_wins() {
        let mut state = State::default();
        state
            .sessions
            .insert(1, make_session(1, Activity::Thinking, 100));

        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Done, 200));
        state.merge_sessions(incoming);

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Done);
        assert_eq!(session.last_event_ts, 200);
    }

    #[test]
    fn test_merge_sessions_older_loses() {
        let mut state = State::default();
        state
            .sessions
            .insert(1, make_session(1, Activity::Done, 200));

        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));
        state.merge_sessions(incoming);

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Done);
        assert_eq!(session.last_event_ts, 200);
    }

    #[test]
    fn test_merge_sessions_equal_timestamp_replaces() {
        let mut state = State::default();
        state
            .sessions
            .insert(1, make_session(1, Activity::Thinking, 100));

        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Done, 100));
        state.merge_sessions(incoming);

        // Equal timestamp: incoming wins (>= comparison)
        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.activity, Activity::Done);
    }

    #[test]
    fn test_merge_sessions_applies_pane_to_tab() {
        let mut state = State::default();
        state.pane_to_tab.insert(1, (0, "Tab 1".into()));

        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));
        state.merge_sessions(incoming);

        let session = state.sessions.get(&1).unwrap();
        assert_eq!(session.tab_index, Some(0));
        assert_eq!(session.tab_name, Some("Tab 1".into()));
    }

    #[test]
    fn test_merge_sessions_multiple_panes() {
        let mut state = State::default();
        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));
        incoming.insert(2, make_session(2, Activity::Done, 200));
        state.merge_sessions(incoming);

        assert!(state.sessions.get(&1).is_some());
        assert!(state.sessions.get(&2).is_some());
    }

    #[test]
    fn test_sessions_round_trip_serialization() {
        let mut sessions = BTreeMap::new();
        sessions.insert(1, make_session(1, Activity::Thinking, 100));
        sessions.insert(2, make_session(2, Activity::Done, 200));

        let json = serde_json::to_string(&sessions).unwrap();
        let deserialized: BTreeMap<u32, SessionInfo> = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized.get(&1).unwrap().activity, Activity::Thinking);
        assert_eq!(deserialized.get(&1).unwrap().last_event_ts, 100);
        assert_eq!(deserialized.get(&2).unwrap().activity, Activity::Done);
        assert_eq!(deserialized.get(&2).unwrap().last_event_ts, 200);
    }

    #[test]
    fn test_sessions_round_trip_with_all_activity_types() {
        let mut sessions = BTreeMap::new();
        sessions.insert(1, make_session(1, Activity::Init, 10));
        sessions.insert(2, make_session(2, Activity::Thinking, 20));
        sessions.insert(3, make_session(3, Activity::Tool("Bash".into()), 30));
        sessions.insert(4, make_session(4, Activity::Done, 40));
        sessions.insert(5, make_session(5, Activity::Idle, 50));

        let json = serde_json::to_string(&sessions).unwrap();
        let deserialized: BTreeMap<u32, SessionInfo> = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.len(), 5);
        assert_eq!(deserialized.get(&1).unwrap().activity, Activity::Init);
        assert_eq!(deserialized.get(&2).unwrap().activity, Activity::Thinking);
        assert_eq!(
            deserialized.get(&3).unwrap().activity,
            Activity::Tool("Bash".into())
        );
        assert_eq!(deserialized.get(&4).unwrap().activity, Activity::Done);
        assert_eq!(deserialized.get(&5).unwrap().activity, Activity::Idle);
    }
}
