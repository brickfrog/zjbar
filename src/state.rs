use crate::choir_status::ChoirStatus;
use crate::config::BarConfig;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use zellij_tile::prelude::*;

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub const FLASH_DURATION_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Activity {
    Init,
    Thinking,
    Tool(String),
    Prompting,
    Waiting,
    Notification,
    Done,
    AgentDone,
    Idle,
}

impl Activity {
    /// Check if transitioning from `self` to `target` is a valid state transition.
    /// Returns true for valid transitions, false for unexpected ones.
    /// NOTE: This is permissive — callers should log but still apply unexpected transitions
    /// to avoid breaking different AI tool event sequences (Codex only sends Stop,
    /// OpenCode skips PostToolUse, etc.).
    pub fn can_transition_to(&self, target: &Activity) -> bool {
        use Activity::*;
        match (self, target) {
            // SessionStart → Init from any state (re-initialization)
            (_, Init) => true,
            // Thinking can be entered from most active states
            (Idle | Init | Done | AgentDone | Thinking | Tool(_), Thinking) => true,
            // Tool usage from Thinking or another Tool, or directly from Init (OpenCode skips Thinking)
            (Thinking | Tool(_) | Init, Tool(_)) => true,
            // Waiting/Notification from active processing states
            (Thinking | Tool(_), Waiting | Notification) => true,
            // Stop → Done from any active state
            (Init | Thinking | Tool(_) | Prompting | Waiting | Notification, Done) => true,
            // Done timeout → Idle, Done → AgentDone
            (Done, Idle | AgentDone) => true,
            // AgentDone timeout → Idle
            (AgentDone, Idle) => true,
            // Prompting from user input
            (Idle | Done | AgentDone | Thinking, Prompting) => true,
            // Allow Idle → Done (Codex only sends Stop, may be in Idle state)
            (Idle, Done) => true,
            // Everything else is unexpected
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub pane_id: u32,
    pub activity: Activity,
    pub tab_name: Option<String>,
    pub tab_index: Option<usize>,
    pub last_event_ts: u64,
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HookPayload {
    #[allow(dead_code)]
    pub source: Option<String>,
    pub session_id: Option<String>,
    pub pane_id: u32,
    pub hook_event: String,
    pub tool_name: Option<String>,
    pub cwd: Option<String>,
    pub zellij_session: Option<String>,
    pub term_program: Option<String>,
}

impl HookPayload {
    /// Validate required fields. Returns an error message if validation fails, None if valid.
    pub fn validate(&self) -> Option<&'static str> {
        if self.hook_event.is_empty() {
            return Some("hook_event is empty");
        }
        None
    }
}

// -- Flash mode --

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashMode {
    #[default]
    Brief,
    Persist,
    Off,
}

impl FlashMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Brief => Self::Persist,
            Self::Persist => Self::Off,
            Self::Off => Self::Brief,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Persist => "persist",
            Self::Off => "off",
        }
    }
}

// -- Notify mode --

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyMode {
    #[default]
    Always,
    Unfocused,
    Off,
}

impl NotifyMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Always => Self::Unfocused,
            Self::Unfocused => Self::Off,
            Self::Off => Self::Always,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Unfocused => "unfocused",
            Self::Off => "off",
        }
    }
}

// -- Persisted settings (stored in zjbar.json) --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub flash: FlashMode,
    pub elapsed_time: bool,
    pub notifications: NotifyMode,
    pub notify_events: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            flash: FlashMode::Brief,
            elapsed_time: true,
            notifications: NotifyMode::Always,
            notify_events: vec![
                "PermissionRequest".into(),
                "Notification".into(),
                "Stop".into(),
            ],
        }
    }
}

// -- Settings menu types --

#[derive(Default, PartialEq)]
pub enum ViewMode {
    #[default]
    Normal,
    Settings,
}

pub enum SettingKey {
    Flash,
    ElapsedTime,
    Notifications,
}

pub enum MenuAction {
    ToggleSetting(SettingKey),
    CloseMenu,
}

pub struct MenuClickRegion {
    pub start_col: usize,
    pub end_col: usize,
    pub action: MenuAction,
}

// -- Tab click regions --

pub struct ClickRegion {
    pub row: isize,
    pub start_col: usize,
    pub end_col: usize,
    pub tab_index: usize,
    pub pane_id: u32,
    pub is_waiting: bool,
}

// -- Plugin state --

#[derive(Default)]
pub struct State {
    pub config: BarConfig,
    pub choir_status: ChoirStatus,
    pub choir_poll_inflight: bool,
    pub choir_last_poll_ms: u64,
    pub choir_last_ready_ms: u64,
    pub settings: Settings,
    pub view_mode: ViewMode,
    pub menu_click_regions: Vec<MenuClickRegion>,
    pub prefix_click_region: Option<(usize, usize)>,
    pub config_loaded: bool,
    pub sessions: BTreeMap<u32, SessionInfo>,
    pub pane_to_tab: HashMap<u32, (usize, String)>,
    pub pane_titles: HashMap<u32, String>,
    pub tabs: Vec<TabInfo>,
    pub pane_manifest: Option<PaneManifest>,
    pub active_tab_index: Option<usize>,
    pub click_regions: Vec<ClickRegion>,
    /// pane_id -> flash deadline in ms (for waiting animation)
    pub flash_deadlines: HashMap<u32, u64>,
    pub attention_panes: HashSet<u32>,
    pub zellij_session_name: Option<String>,
    pub term_program: Option<String>,
    pub input_mode: InputMode,
    pub initial_cwd: Option<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_init_from_any_state() {
        // SessionStart → Init should be valid from any state
        assert!(Activity::Idle.can_transition_to(&Activity::Init));
        assert!(Activity::Thinking.can_transition_to(&Activity::Init));
        assert!(Activity::Done.can_transition_to(&Activity::Init));
        assert!(Activity::Tool("Bash".into()).can_transition_to(&Activity::Init));
    }

    #[test]
    fn transition_thinking_from_valid_states() {
        assert!(Activity::Idle.can_transition_to(&Activity::Thinking));
        assert!(Activity::Init.can_transition_to(&Activity::Thinking));
        assert!(Activity::Done.can_transition_to(&Activity::Thinking));
        assert!(Activity::Tool("Bash".into()).can_transition_to(&Activity::Thinking));
    }

    #[test]
    fn transition_tool_from_thinking() {
        assert!(Activity::Thinking.can_transition_to(&Activity::Tool("Bash".into())));
        assert!(Activity::Tool("Read".into()).can_transition_to(&Activity::Tool("Write".into())));
        assert!(Activity::Init.can_transition_to(&Activity::Tool("Bash".into())));
    }

    #[test]
    fn transition_done_from_active_states() {
        assert!(Activity::Thinking.can_transition_to(&Activity::Done));
        assert!(Activity::Tool("Bash".into()).can_transition_to(&Activity::Done));
        assert!(Activity::Waiting.can_transition_to(&Activity::Done));
        assert!(Activity::Idle.can_transition_to(&Activity::Done)); // Codex edge case
    }

    #[test]
    fn transition_done_to_idle() {
        assert!(Activity::Done.can_transition_to(&Activity::Idle));
        assert!(Activity::AgentDone.can_transition_to(&Activity::Idle));
    }

    #[test]
    fn transition_unexpected_logged_not_blocked() {
        // Waiting → Prompting is unexpected but should be identifiable
        assert!(!Activity::Waiting.can_transition_to(&Activity::Prompting));
        // Notification → Tool is unexpected
        assert!(!Activity::Notification.can_transition_to(&Activity::Tool("Bash".into())));
    }

    #[test]
    fn validate_payload_empty_hook_event() {
        let payload = HookPayload {
            source: Some("claude".into()),
            session_id: Some("test".into()),
            pane_id: 1,
            hook_event: "".into(),
            tool_name: None,
            cwd: None,
            zellij_session: None,
            term_program: None,
        };
        assert_eq!(payload.validate(), Some("hook_event is empty"));
    }

    #[test]
    fn validate_payload_valid() {
        let payload = HookPayload {
            source: Some("claude".into()),
            session_id: Some("test".into()),
            pane_id: 1,
            hook_event: "PreToolUse".into(),
            tool_name: Some("Bash".into()),
            cwd: None,
            zellij_session: None,
            term_program: None,
        };
        assert_eq!(payload.validate(), None);
    }

    #[test]
    fn test_transition_prompting_from_valid_states() {
        assert!(Activity::Idle.can_transition_to(&Activity::Prompting));
        assert!(Activity::Done.can_transition_to(&Activity::Prompting));
        assert!(Activity::Thinking.can_transition_to(&Activity::Prompting));
    }

    #[test]
    fn test_transition_waiting_from_valid_states() {
        assert!(Activity::Thinking.can_transition_to(&Activity::Waiting));
        assert!(Activity::Tool("Bash".into()).can_transition_to(&Activity::Waiting));
    }

    #[test]
    fn test_transition_agent_done_from_done() {
        assert!(Activity::Done.can_transition_to(&Activity::AgentDone));
    }

    #[test]
    fn test_transition_invalid_paths() {
        // Idle → Tool should be invalid (must go through Thinking/Init first)
        assert!(!Activity::Idle.can_transition_to(&Activity::Tool("Bash".into())));
        // AgentDone → Thinking should be valid (re-engagement)
        assert!(Activity::AgentDone.can_transition_to(&Activity::Thinking));
        // Waiting → Tool should be invalid
        assert!(!Activity::Waiting.can_transition_to(&Activity::Tool("X".into())));
        // Notification → Prompting should be invalid
        assert!(!Activity::Notification.can_transition_to(&Activity::Prompting));
    }
}
