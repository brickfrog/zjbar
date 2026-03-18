use crate::config::BarConfig;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
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

// -- Flash mode --

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashMode {
    Brief,
    Persist,
    Off,
}

impl Default for FlashMode {
    fn default() -> Self {
        Self::Brief
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyMode {
    Always,
    Unfocused,
    Off,
}

impl Default for NotifyMode {
    fn default() -> Self {
        Self::Always
    }
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
    pub settings: Settings,
    pub view_mode: ViewMode,
    pub menu_click_regions: Vec<MenuClickRegion>,
    pub prefix_click_region: Option<(usize, usize)>,
    pub config_loaded: bool,
    pub sessions: BTreeMap<u32, SessionInfo>,
    pub pane_to_tab: HashMap<u32, (usize, String)>,
    pub tabs: Vec<TabInfo>,
    pub pane_manifest: Option<PaneManifest>,
    pub active_tab_index: Option<usize>,
    pub click_regions: Vec<ClickRegion>,
    /// pane_id -> flash deadline in ms (for waiting animation)
    pub flash_deadlines: HashMap<u32, u64>,
    pub zellij_session_name: Option<String>,
    pub term_program: Option<String>,
    pub input_mode: InputMode,
}
