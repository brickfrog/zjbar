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
const CHOIR_NO_CHOIR_POLL_INTERVAL_MS: u64 = 10_000;
const CHOIR_ERROR_POLL_INTERVAL_MS: u64 = 5_000;

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = BarConfig::from_kdl(&configuration);
        let plugin_ids = get_plugin_ids();
        self.plugin_id = Some(plugin_ids.plugin_id);
        self.initial_cwd = Some(plugin_ids.initial_cwd);

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
                let mut changed = self.tabs != tabs || new_active != self.active_tab_index;
                if new_active != self.active_tab_index {
                    if let Some(idx) = new_active {
                        self.clear_flashes_on_tab(idx);
                    }
                }
                self.active_tab_index = new_active;
                self.tabs = tabs;
                changed |= self.rebuild_pane_map();
                changed
            }
            Event::PaneUpdate(manifest) => {
                let changed = self.pane_manifest.as_ref() != Some(&manifest);
                self.pane_manifest = Some(manifest);
                self.rebuild_pane_map() || changed
            }
            Event::ModeUpdate(mode_info) => {
                let mut changed = self.input_mode != mode_info.mode;
                self.input_mode = mode_info.mode;
                if let Some(name) = mode_info.session_name {
                    changed |= self.zellij_session_name.as_ref() != Some(&name);
                    self.zellij_session_name = Some(name);
                }
                changed
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
                let changed = match context.get("type").map(|s| s.as_str()) {
                    Some("choir_status_poll") => {
                        self.choir_poll_inflight = false;
                        if exit_code == Some(0) {
                            let raw = String::from_utf8_lossy(&stdout);
                            match choir_status::parse_status_bar_state_response(raw.trim()) {
                                Ok(snapshot) => {
                                    self.schedule_next_choir_poll(CHOIR_POLL_INTERVAL_MS);
                                    self.apply_choir_snapshot(snapshot)
                                }
                                Err(StatusBarError::SchemaAhead(version)) => {
                                    self.schedule_next_choir_poll(CHOIR_ERROR_POLL_INTERVAL_MS);
                                    self.apply_choir_status_error(ChoirStatus::SchemaAhead(version))
                                }
                                Err(e) => {
                                    eprintln!("[zjbar] failed to parse choir status: {e}");
                                    self.schedule_next_choir_poll(CHOIR_ERROR_POLL_INTERVAL_MS);
                                    self.apply_choir_status_error(ChoirStatus::Invalid(
                                        e.to_string(),
                                    ))
                                }
                            }
                        } else {
                            self.schedule_next_choir_poll(CHOIR_NO_CHOIR_POLL_INTERVAL_MS);
                            self.apply_choir_status_error(ChoirStatus::NoChoir)
                        }
                    }
                    Some("load_config") if exit_code == Some(0) => {
                        let raw = String::from_utf8_lossy(&stdout);
                        let was_loaded = self.config_loaded;
                        let mut changed = false;
                        match serde_json::from_str::<Settings>(raw.trim()) {
                            Ok(settings) => {
                                if self.settings != settings {
                                    self.settings = settings;
                                    changed = true;
                                }
                            }
                            Err(e) => {
                                eprintln!("[zjbar] failed to parse config file: {e}");
                            }
                        }
                        self.config_loaded = true;
                        changed || !was_loaded
                    }
                    _ => false,
                };
                changed
            }
            Event::PermissionRequestResult(_) => {
                set_selectable(false);
                if !self.config_loaded {
                    self.load_config();
                }
                self.poll_choir_status_if_due();
                if !self.sync_requested {
                    self.sync_requested = true;
                    self.request_sync();
                }
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
                if !self.owns_hook_payload(&payload) {
                    return false;
                }
                if event_handler::handle_hook_event(self, payload) {
                    self.broadcast_sessions();
                    true
                } else {
                    false
                }
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
                if self.owns_sync_response() {
                    self.broadcast_sessions();
                    self.broadcast_choir_status();
                }
                false
            }
            "zjbar:sync" => {
                if let Some(ref payload) = pipe_message.payload {
                    match serde_json::from_str::<BTreeMap<u32, SessionInfo>>(payload) {
                        Ok(sessions) => {
                            return self.merge_sessions(sessions);
                        }
                        Err(e) => {
                            eprintln!("[zjbar] failed to parse sync payload: {e}");
                        }
                    }
                }
                false
            }
            "zjbar:choir_status" => {
                if let Some(ref payload) = pipe_message.payload {
                    match serde_json::from_str::<StatusBarState>(payload) {
                        Ok(snapshot) => {
                            return self.merge_choir_snapshot(snapshot);
                        }
                        Err(e) => {
                            eprintln!("[zjbar] failed to parse choir status sync payload: {e}");
                        }
                    }
                }
                false
            }
            "zjbar:settings" => {
                if let Some(ref payload) = pipe_message.payload {
                    match serde_json::from_str::<Settings>(payload) {
                        Ok(settings) => {
                            if self.settings != settings {
                                self.settings = settings;
                                self.config_loaded = true;
                                return true;
                            }
                            self.config_loaded = true;
                            return false;
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
    fn owns_global_work(&self) -> bool {
        self.own_tab_index.is_some() && self.own_tab_index == self.active_tab_index
    }

    fn owns_sync_response(&self) -> bool {
        let first_tab = self.tabs.iter().map(|tab| tab.position).min();
        self.own_tab_index.is_some() && self.own_tab_index == first_tab
    }

    fn owns_hook_payload(&self, payload: &HookPayload) -> bool {
        match (self.pane_to_tab.get(&payload.pane_id), self.own_tab_index) {
            (Some((tab_index, _)), Some(own_tab_index)) => *tab_index == own_tab_index,
            _ => self.owns_sync_response(),
        }
    }

    fn schedule_next_choir_poll(&mut self, interval_ms: u64) {
        self.choir_next_poll_ms = unix_now_ms().saturating_add(interval_ms);
    }

    fn poll_choir_status_if_due(&mut self) -> bool {
        if !self.owns_global_work() {
            return false;
        }
        if self.choir_poll_inflight {
            return false;
        }
        let now = unix_now_ms();
        if self.choir_next_poll_ms != 0 && now < self.choir_next_poll_ms {
            return false;
        }
        if self.choir_last_poll_ms != 0
            && now.saturating_sub(self.choir_last_poll_ms) < CHOIR_POLL_INTERVAL_MS
        {
            return false;
        }
        self.choir_last_poll_ms = now;
        self.choir_poll_inflight = true;
        let mut ctx = BTreeMap::new();
        ctx.insert("type".into(), "choir_status_poll".into());
        let initial_cwd = self
            .initial_cwd
            .as_ref()
            .map(|cwd| cwd.to_string_lossy().into_owned());
        let cmd = choir_status::poll_command(&self.config.choir_socket, initial_cwd.as_deref());
        run_command(&["sh", "-c", &cmd], ctx);
        false
    }

    fn merge_choir_snapshot(&mut self, snapshot: StatusBarState) -> bool {
        let should_apply = match &self.choir_status {
            ChoirStatus::Ready(existing) => {
                if snapshot.taken_at_ms < existing.taken_at_ms {
                    false
                } else {
                    snapshot.taken_at_ms > existing.taken_at_ms || snapshot != *existing
                }
            }
            _ => true,
        };
        if !should_apply {
            return false;
        }
        let now = unix_now_ms();
        let next_attention: HashSet<u32> = snapshot
            .panes
            .iter()
            .filter(|pane| pane.attention_needed.is_needed())
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
        self.choir_last_ready_ms = now;
        true
    }

    fn apply_choir_snapshot(&mut self, snapshot: StatusBarState) -> bool {
        let changed = self.merge_choir_snapshot(snapshot);
        if changed {
            self.broadcast_choir_status();
        }
        changed
    }

    fn apply_choir_status_error(&mut self, status: ChoirStatus) -> bool {
        if matches!(status, ChoirStatus::NoChoir)
            && self.choir_last_ready_ms != 0
            && unix_now_ms().saturating_sub(self.choir_last_ready_ms) < CHOIR_POLL_INTERVAL_MS * 5
        {
            return false;
        }
        let changed = self.choir_status != status
            || !self.attention_panes.is_empty()
            || !self.flash_deadlines.is_empty();
        self.attention_panes.clear();
        self.flash_deadlines.clear();
        self.choir_status = status;
        changed
    }

    fn rebuild_pane_map(&mut self) -> bool {
        if let Some(ref manifest) = self.pane_manifest {
            let next_pane_to_tab = tab_pane_map::build_pane_to_tab_map(&self.tabs, manifest);
            let next_pane_titles = tab_pane_map::build_pane_title_map(manifest);
            let next_own_tab = self.plugin_id.and_then(|plugin_id| {
                manifest.panes.iter().find_map(|(&tab_index, panes)| {
                    panes
                        .iter()
                        .any(|pane| pane.is_plugin && pane.id == plugin_id)
                        .then_some(tab_index)
                })
            });

            let mut changed = self.pane_to_tab != next_pane_to_tab
                || self.pane_titles != next_pane_titles
                || self.own_tab_index != next_own_tab;
            self.pane_to_tab = next_pane_to_tab;
            self.pane_titles = next_pane_titles;
            self.own_tab_index = next_own_tab;
            changed |= self.refresh_session_tab_names();
            changed |= self.remove_dead_panes();
            return changed;
        }
        false
    }

    fn refresh_session_tab_names(&mut self) -> bool {
        let mut changed = false;
        for session in self.sessions.values_mut() {
            if let Some((idx, name)) = self.pane_to_tab.get(&session.pane_id) {
                changed |=
                    session.tab_index != Some(*idx) || session.tab_name.as_ref() != Some(name);
                session.tab_index = Some(*idx);
                session.tab_name = Some(name.clone());
            }
        }
        changed
    }

    fn remove_dead_panes(&mut self) -> bool {
        let before = self.sessions.len();
        self.sessions
            .retain(|pane_id, _| self.pane_to_tab.contains_key(pane_id));
        self.sessions.len() != before
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

    fn broadcast_choir_status(&self) {
        let ChoirStatus::Ready(snapshot) = &self.choir_status else {
            return;
        };
        let mut msg = MessageToPlugin::new("zjbar:choir_status");
        msg.message_payload = Some(match serde_json::to_string(snapshot) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("[zjbar] failed to serialize choir status for sync: {e}");
                return;
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

    fn merge_sessions(&mut self, incoming: BTreeMap<u32, SessionInfo>) -> bool {
        let mut changed = false;
        for (pane_id, mut session) in incoming {
            if let Some((idx, name)) = self.pane_to_tab.get(&pane_id) {
                session.tab_index = Some(*idx);
                session.tab_name = Some(name.clone());
            }
            match self.sessions.get(&pane_id) {
                Some(existing) if session.last_event_ts < existing.last_event_ts => continue,
                Some(existing) if *existing == session => continue,
                _ => {
                    self.sessions.insert(pane_id, session);
                    changed = true;
                }
            }
        }
        changed
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

    fn make_choir_snapshot(taken_at_ms: u64, lifecycle: choir_status::Lifecycle) -> StatusBarState {
        StatusBarState {
            schema_version: choir_status::STATUS_BAR_SCHEMA_VERSION,
            taken_at_ms,
            panes: vec![choir_status::StatusBarPane {
                zellij_pane_id: 1,
                agent_id: "root".into(),
                role: choir_status::ChoirRole::Tl,
                agent_type: choir_status::AgentType::Codex,
                lifecycle,
                pr_number: None,
                unresolved_threads: choir_status::UnresolvedThreads::Count(0),
                ci_rollup: choir_status::CiRollup::Unknown,
                attention_needed: choir_status::PaneAttention::Clear,
                parent_agent_id: None,
                last_activity_unix: 1,
            }],
        }
    }

    fn make_tab(position: usize, active: bool) -> TabInfo {
        TabInfo {
            position,
            name: format!("Tab {}", position + 1),
            active,
            ..Default::default()
        }
    }

    fn make_pane(id: u32, is_plugin: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin,
            ..Default::default()
        }
    }

    fn make_manifest(entries: Vec<(usize, Vec<PaneInfo>)>) -> PaneManifest {
        PaneManifest {
            panes: entries.into_iter().collect(),
        }
    }

    fn make_payload(pane_id: u32) -> HookPayload {
        HookPayload {
            source: Some("claude".into()),
            session_id: Some("test-session".into()),
            pane_id,
            hook_event: "Stop".into(),
            tool_name: None,
            cwd: None,
            zellij_session: None,
            term_program: None,
        }
    }

    #[test]
    fn test_merge_choir_snapshot_newer_wins() {
        let mut state = State::default();
        assert!(
            state.merge_choir_snapshot(make_choir_snapshot(200, choir_status::Lifecycle::Working,))
        );
        assert!(
            !state.merge_choir_snapshot(make_choir_snapshot(100, choir_status::Lifecycle::Done,))
        );

        let ChoirStatus::Ready(snapshot) = state.choir_status else {
            panic!("snapshot should remain ready");
        };
        assert_eq!(snapshot.taken_at_ms, 200);
        assert_eq!(
            snapshot.panes[0].lifecycle,
            choir_status::Lifecycle::Working
        );
    }

    #[test]
    fn test_merge_choir_snapshot_identical_is_noop() {
        let mut state = State::default();
        let snapshot = make_choir_snapshot(200, choir_status::Lifecycle::Working);

        assert!(state.merge_choir_snapshot(snapshot.clone()));
        assert!(!state.merge_choir_snapshot(snapshot));
    }

    #[test]
    fn test_recent_choir_snapshot_ignores_local_no_choir_error() {
        let mut state = State::default();
        assert!(
            state.merge_choir_snapshot(make_choir_snapshot(100, choir_status::Lifecycle::Working,))
        );

        state.apply_choir_status_error(ChoirStatus::NoChoir);

        assert!(matches!(state.choir_status, ChoirStatus::Ready(_)));
    }

    #[test]
    fn test_merge_sessions_new_entry() {
        let mut state = State::default();
        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));
        assert!(state.merge_sessions(incoming));

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
        assert!(state.merge_sessions(incoming));

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
        assert!(!state.merge_sessions(incoming));

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
        assert!(state.merge_sessions(incoming));

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
        assert!(state.merge_sessions(incoming));

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
        assert!(state.merge_sessions(incoming));

        assert!(state.sessions.get(&1).is_some());
        assert!(state.sessions.get(&2).is_some());
    }

    #[test]
    fn test_merge_sessions_identical_is_noop() {
        let mut state = State::default();
        state
            .sessions
            .insert(1, make_session(1, Activity::Thinking, 100));

        let mut incoming = BTreeMap::new();
        incoming.insert(1, make_session(1, Activity::Thinking, 100));

        assert!(!state.merge_sessions(incoming));
    }

    #[test]
    fn test_rebuild_pane_map_discovers_active_plugin_owner() {
        let mut state = State::default();
        state.plugin_id = Some(42);
        state.active_tab_index = Some(1);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert_eq!(state.own_tab_index, Some(1));
        assert!(state.owns_global_work());
    }

    #[test]
    fn test_inactive_plugin_instance_does_not_own_global_work() {
        let mut state = State::default();
        state.plugin_id = Some(42);
        state.active_tab_index = Some(0);
        state.tabs = vec![make_tab(0, true), make_tab(1, false)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert_eq!(state.own_tab_index, Some(1));
        assert!(!state.owns_global_work());
    }

    #[test]
    fn test_first_tab_plugin_instance_owns_sync_response() {
        let mut state = State::default();
        state.plugin_id = Some(10);
        state.active_tab_index = Some(1);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert_eq!(state.own_tab_index, Some(0));
        assert!(state.owns_sync_response());
        assert!(!state.owns_global_work());
    }

    #[test]
    fn test_non_first_tab_plugin_instance_does_not_own_sync_response() {
        let mut state = State::default();
        state.plugin_id = Some(42);
        state.active_tab_index = Some(1);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert_eq!(state.own_tab_index, Some(1));
        assert!(!state.owns_sync_response());
        assert!(state.owns_global_work());
    }

    #[test]
    fn test_tab_plugin_instance_owns_hook_payload_for_its_pane() {
        let mut state = State::default();
        state.plugin_id = Some(42);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert!(state.owns_hook_payload(&make_payload(12)));
    }

    #[test]
    fn test_other_tab_plugin_instance_ignores_hook_payload() {
        let mut state = State::default();
        state.plugin_id = Some(10);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert!(!state.owns_hook_payload(&make_payload(12)));
    }

    #[test]
    fn test_first_tab_plugin_instance_handles_unmapped_hook_payload() {
        let mut state = State::default();
        state.plugin_id = Some(10);
        state.tabs = vec![make_tab(0, false), make_tab(1, true)];
        state.pane_manifest = Some(make_manifest(vec![
            (0, vec![make_pane(10, true), make_pane(11, false)]),
            (1, vec![make_pane(42, true), make_pane(12, false)]),
        ]));

        assert!(state.rebuild_pane_map());
        assert!(state.owns_hook_payload(&make_payload(99)));
    }

    #[test]
    fn test_no_choir_backoff_is_slower_than_regular_poll() {
        assert!(CHOIR_NO_CHOIR_POLL_INTERVAL_MS > CHOIR_POLL_INTERVAL_MS);
    }

    #[test]
    fn test_repeated_no_choir_error_is_noop() {
        let mut state = State::default();

        assert!(!state.apply_choir_status_error(ChoirStatus::NoChoir));
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
