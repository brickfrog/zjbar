mod config;
mod event_handler;
mod render;
mod state;
mod tab_pane_map;

use config::BarConfig;
use state::{
    unix_now, unix_now_ms, HookPayload, MenuAction, SessionInfo, Settings, SettingKey, State,
    ViewMode,
};
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

const DONE_TIMEOUT: u64 = 30;
const TIMER_INTERVAL: f64 = 1.0;
const FLASH_TICK: f64 = 0.25;

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = BarConfig::from_kdl(&configuration);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::Timer,
            EventType::Mouse,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
        ]);
        set_timeout(TIMER_INTERVAL);
        self.load_config();
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
            Event::Mouse(Mouse::LeftClick(_, col)) => {
                let col = col as usize;

                // Check prefix click → toggle settings menu
                if let Some((start, end)) = self.prefix_click_region {
                    if col >= start && col < end {
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
                            if col >= region.start_col && col < region.end_col {
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
                                                self.settings.flash =
                                                    self.settings.flash.cycle();
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
                let has_flashes = self.has_active_flashes();
                if has_flashes {
                    set_timeout(FLASH_TICK);
                } else {
                    set_timeout(TIMER_INTERVAL);
                }
                has_flashes || stale_changed || flash_changed || self.has_elapsed_display()
            }
            Event::RunCommandResult(exit_code, stdout, _stderr, context) => {
                match context.get("type").map(|s| s.as_str()) {
                    Some("load_config") if exit_code == Some(0) => {
                        let raw = String::from_utf8_lossy(&stdout);
                        if let Ok(settings) = serde_json::from_str::<Settings>(raw.trim()) {
                            self.settings = settings;
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
                    Err(_) => return false,
                };
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
                    if let Ok(sessions) =
                        serde_json::from_str::<BTreeMap<u32, SessionInfo>>(payload)
                    {
                        self.merge_sessions(sessions);
                        return true;
                    }
                }
                false
            }
            "zjbar:settings" => {
                if let Some(ref payload) = pipe_message.payload {
                    if let Ok(settings) = serde_json::from_str::<Settings>(payload) {
                        self.settings = settings;
                        self.config_loaded = true;
                        return true;
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
        let mut suspending_done_panes: Vec<u32> = Vec::new();
        for session in self.sessions.values_mut() {
            match session.activity {
                state::Activity::Suspending => {
                    // Suspending → Done after SUSPENDING_TIMEOUT (fallback if
                    // SubagentStop never arrives).
                    if let Some(started) = session.suspending_started_at {
                        if now.saturating_sub(started) >= state::SUSPENDING_TIMEOUT {
                            session.activity = state::Activity::Done;
                            session.suspending_started_at = None;
                            session.last_event_ts = now;
                            suspending_done_panes.push(session.pane_id);
                            changed = true;
                        }
                    }
                }
                state::Activity::Done | state::Activity::AgentDone => {
                    if now.saturating_sub(session.last_event_ts) >= DONE_TIMEOUT {
                        session.activity = state::Activity::Idle;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        // Fire desktop notification for Suspending → Done transitions.
        // The shell hook couldn't send it at idle_prompt time because we
        // were still waiting for SubagentStop.
        for pane_id in suspending_done_panes {
            self.fire_suspending_done_notification(pane_id);
        }
        changed
    }

    /// Send a desktop notification when a Suspending session times out to Done.
    /// This compensates for the shell hook not sending a notification at
    /// idle_prompt time (it only sends Suspending to Rust, not Stop).
    fn fire_suspending_done_notification(&self, _pane_id: u32) {
        let app_name = if self.term_program.as_deref() == Some("codebuddy") {
            "CodeBuddy"
        } else {
            "Claude Code"
        };
        let title = format!("✅ {}", app_name);
        let message = "Task completed";
        let cmd = format!(
            "command -v terminal-notifier >/dev/null 2>&1 && \
             terminal-notifier -title '{}' -message '{}' || \
             command -v notify-send >/dev/null 2>&1 && \
             notify-send '{}' '{}'",
            title, message, title, message
        );
        let mut ctx = BTreeMap::new();
        ctx.insert("type".into(), "suspending_notify".into());
        run_command(&["sh", "-c", &cmd], ctx);
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
        self.flash_deadlines.values().any(|&deadline| now < deadline)
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
        msg.message_payload =
            Some(serde_json::to_string(&self.sessions).unwrap_or_default());
        pipe_message_to_plugin(msg);
    }

    fn broadcast_settings(&self) {
        let mut msg = MessageToPlugin::new("zjbar:settings");
        msg.message_payload =
            Some(serde_json::to_string(&self.settings).unwrap_or_default());
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
        let json = serde_json::to_string(&self.settings).unwrap_or_default();
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
