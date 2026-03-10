use std::collections::BTreeMap;

pub type Color = (u8, u8, u8);

/// Parse "#rrggbb" hex color string into (r, g, b) tuple.
fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

fn get_color(config: &BTreeMap<String, String>, key: &str, default: Color) -> Color {
    config
        .get(key)
        .and_then(|v| parse_hex_color(v))
        .unwrap_or(default)
}

fn get_str<'a>(config: &'a BTreeMap<String, String>, key: &str, default: &'a str) -> &'a str {
    config.get(key).map(|s| s.as_str()).unwrap_or(default)
}


// -- Tokyo Night defaults --
const D_BAR_BG: Color = (26, 27, 38);          // #1a1b26
const D_SESSION_BG: Color = (122, 162, 247);    // #7aa2f7
const D_SESSION_FG: Color = (22, 22, 30);       // #16161e
const D_TAB_ACTIVE_BG: Color = (41, 46, 66);    // #292e42
const D_TAB_ACTIVE_FG: Color = (192, 202, 245); // #c0caf5
const D_TAB_ACTIVE_INDEX_BG: Color = (122, 162, 247); // #7aa2f7
const D_TAB_ACTIVE_INDEX_FG: Color = (22, 22, 30);    // #16161e
const D_TAB_INACTIVE_BG: Color = (22, 22, 30);  // #16161e
const D_TAB_INACTIVE_FG: Color = (169, 177, 214); // #a9b1d6
const D_TAB_SEPARATOR: Color = (86, 95, 137);   // #565f89
const D_FLASH_BG: Color = (80, 70, 30);
const D_FLASH_FG: Color = (224, 175, 104);      // #e0af68

const D_MODE_NORMAL_BG: Color = (158, 206, 106);  // #9ece6a
const D_MODE_LOCKED_BG: Color = (247, 118, 142);  // #f7768e
const D_MODE_PANE_BG: Color = (125, 207, 255);    // #7dcfff
const D_MODE_TAB_BG: Color = (125, 207, 255);     // #7dcfff
const D_MODE_RESIZE_BG: Color = (224, 175, 104);  // #e0af68
const D_MODE_MOVE_BG: Color = (224, 175, 104);    // #e0af68
const D_MODE_SCROLL_BG: Color = (255, 158, 100);  // #ff9e64
const D_MODE_SEARCH_BG: Color = (255, 158, 100);  // #ff9e64
const D_MODE_ENTERSEARCH_BG: Color = (255, 158, 100); // #ff9e64
const D_MODE_SESSION_BG: Color = (187, 154, 247); // #bb9af7
const D_MODE_PROMPT_BG: Color = (187, 154, 247);  // #bb9af7
const D_MODE_RENAMETAB_BG: Color = (224, 175, 104); // #e0af68
const D_MODE_RENAMEPANE_BG: Color = (224, 175, 104); // #e0af68
const D_MODE_TMUX_BG: Color = (122, 162, 247);    // #7aa2f7
const D_MODE_FG: Color = (22, 22, 30);            // #16161e (shared default)

const D_ACTIVITY_INIT: Color = (122, 162, 247);    // #7aa2f7
const D_ACTIVITY_THINKING: Color = (187, 154, 247); // #bb9af7
const D_ACTIVITY_TOOL: Color = (255, 158, 100);    // #ff9e64
const D_ACTIVITY_WAITING: Color = (247, 118, 142); // #f7768e
const D_ACTIVITY_PERMISSION: Color = (247, 118, 142); // #f7768e
const D_ACTIVITY_DONE: Color = (158, 206, 106);    // #9ece6a
const D_ACTIVITY_PROMPT: Color = (158, 206, 106);  // #9ece6a

const D_ELAPSED_FG: Color = (86, 95, 137);         // #565f89

/// All configuration parsed from the KDL plugin block.
#[allow(dead_code)]
pub struct BarConfig {
    // Global
    pub bar_bg: Color,

    // Session pill
    pub session_bg: Color,
    pub session_fg: Color,

    // Tab - active
    pub tab_active_bg: Color,
    pub tab_active_fg: Color,
    pub tab_active_index_bg: Color,
    pub tab_active_index_fg: Color,

    // Tab - inactive
    pub tab_inactive_bg: Color,
    pub tab_inactive_fg: Color,

    // Tab separator (thin arrow between index and name)
    pub tab_separator_fg: Color,

    // Flash
    pub flash_bg: Color,
    pub flash_fg: Color,

    // Mode colors
    pub mode_normal_bg: Color,
    pub mode_normal_fg: Color,
    pub mode_locked_bg: Color,
    pub mode_locked_fg: Color,
    pub mode_pane_bg: Color,
    pub mode_pane_fg: Color,
    pub mode_tab_bg: Color,
    pub mode_tab_fg: Color,
    pub mode_resize_bg: Color,
    pub mode_resize_fg: Color,
    pub mode_move_bg: Color,
    pub mode_move_fg: Color,
    pub mode_scroll_bg: Color,
    pub mode_scroll_fg: Color,
    pub mode_search_bg: Color,
    pub mode_search_fg: Color,
    pub mode_entersearch_bg: Color,
    pub mode_entersearch_fg: Color,
    pub mode_session_bg: Color,
    pub mode_session_fg: Color,
    pub mode_prompt_bg: Color,
    pub mode_prompt_fg: Color,
    pub mode_renametab_bg: Color,
    pub mode_renametab_fg: Color,
    pub mode_renamepane_bg: Color,
    pub mode_renamepane_fg: Color,
    pub mode_tmux_bg: Color,
    pub mode_tmux_fg: Color,

    // Activity icon colors
    pub activity_init_color: Color,
    pub activity_thinking_color: Color,
    pub activity_tool_color: Color,
    pub activity_waiting_color: Color,
    pub activity_permission_color: Color,
    pub activity_done_color: Color,
    pub activity_prompt_color: Color,

    // Elapsed time color
    pub elapsed_fg: Color,

    // Separators
    pub separator_left: String,
    pub separator_right: String,
    pub separator_tab: String,
}

impl BarConfig {
    pub fn from_kdl(config: &BTreeMap<String, String>) -> Self {
        Self {
            bar_bg: get_color(config, "bar_bg", D_BAR_BG),

            session_bg: get_color(config, "session_bg", D_SESSION_BG),
            session_fg: get_color(config, "session_fg", D_SESSION_FG),

            tab_active_bg: get_color(config, "tab_active_bg", D_TAB_ACTIVE_BG),
            tab_active_fg: get_color(config, "tab_active_fg", D_TAB_ACTIVE_FG),
            tab_active_index_bg: get_color(config, "tab_active_index_bg", D_TAB_ACTIVE_INDEX_BG),
            tab_active_index_fg: get_color(config, "tab_active_index_fg", D_TAB_ACTIVE_INDEX_FG),
            tab_inactive_bg: get_color(config, "tab_inactive_bg", D_TAB_INACTIVE_BG),
            tab_inactive_fg: get_color(config, "tab_inactive_fg", D_TAB_INACTIVE_FG),
            tab_separator_fg: get_color(config, "tab_separator_fg", D_TAB_SEPARATOR),

            flash_bg: get_color(config, "flash_bg", D_FLASH_BG),
            flash_fg: get_color(config, "flash_fg", D_FLASH_FG),

            mode_normal_bg: get_color(config, "mode_normal_bg", D_MODE_NORMAL_BG),
            mode_normal_fg: get_color(config, "mode_normal_fg", D_MODE_FG),
            mode_locked_bg: get_color(config, "mode_locked_bg", D_MODE_LOCKED_BG),
            mode_locked_fg: get_color(config, "mode_locked_fg", D_MODE_FG),
            mode_pane_bg: get_color(config, "mode_pane_bg", D_MODE_PANE_BG),
            mode_pane_fg: get_color(config, "mode_pane_fg", D_MODE_FG),
            mode_tab_bg: get_color(config, "mode_tab_bg", D_MODE_TAB_BG),
            mode_tab_fg: get_color(config, "mode_tab_fg", D_MODE_FG),
            mode_resize_bg: get_color(config, "mode_resize_bg", D_MODE_RESIZE_BG),
            mode_resize_fg: get_color(config, "mode_resize_fg", D_MODE_FG),
            mode_move_bg: get_color(config, "mode_move_bg", D_MODE_MOVE_BG),
            mode_move_fg: get_color(config, "mode_move_fg", D_MODE_FG),
            mode_scroll_bg: get_color(config, "mode_scroll_bg", D_MODE_SCROLL_BG),
            mode_scroll_fg: get_color(config, "mode_scroll_fg", D_MODE_FG),
            mode_search_bg: get_color(config, "mode_search_bg", D_MODE_SEARCH_BG),
            mode_search_fg: get_color(config, "mode_search_fg", D_MODE_FG),
            mode_entersearch_bg: get_color(config, "mode_entersearch_bg", D_MODE_ENTERSEARCH_BG),
            mode_entersearch_fg: get_color(config, "mode_entersearch_fg", D_MODE_FG),
            mode_session_bg: get_color(config, "mode_session_bg", D_MODE_SESSION_BG),
            mode_session_fg: get_color(config, "mode_session_fg", D_MODE_FG),
            mode_prompt_bg: get_color(config, "mode_prompt_bg", D_MODE_PROMPT_BG),
            mode_prompt_fg: get_color(config, "mode_prompt_fg", D_MODE_FG),
            mode_renametab_bg: get_color(config, "mode_renametab_bg", D_MODE_RENAMETAB_BG),
            mode_renametab_fg: get_color(config, "mode_renametab_fg", D_MODE_FG),
            mode_renamepane_bg: get_color(config, "mode_renamepane_bg", D_MODE_RENAMEPANE_BG),
            mode_renamepane_fg: get_color(config, "mode_renamepane_fg", D_MODE_FG),
            mode_tmux_bg: get_color(config, "mode_tmux_bg", D_MODE_TMUX_BG),
            mode_tmux_fg: get_color(config, "mode_tmux_fg", D_MODE_FG),

            activity_init_color: get_color(config, "activity_init_color", D_ACTIVITY_INIT),
            activity_thinking_color: get_color(config, "activity_thinking_color", D_ACTIVITY_THINKING),
            activity_tool_color: get_color(config, "activity_tool_color", D_ACTIVITY_TOOL),
            activity_waiting_color: get_color(config, "activity_waiting_color", D_ACTIVITY_WAITING),
            activity_permission_color: get_color(config, "activity_permission_color", D_ACTIVITY_PERMISSION),
            activity_done_color: get_color(config, "activity_done_color", D_ACTIVITY_DONE),
            activity_prompt_color: get_color(config, "activity_prompt_color", D_ACTIVITY_PROMPT),

            elapsed_fg: get_color(config, "elapsed_fg", D_ELAPSED_FG),

            separator_left: get_str(config, "separator_left", "\u{e0b0}").to_string(),
            separator_right: get_str(config, "separator_right", "\u{e0b2}").to_string(),
            separator_tab: get_str(config, "separator_tab", "\u{e0b1}").to_string(),
        }
    }

    /// Get mode background and foreground colors for the given input mode.
    pub fn mode_style(&self, mode: zellij_tile::prelude::InputMode) -> (Color, Color, &'static str) {
        use zellij_tile::prelude::InputMode;
        match mode {
            InputMode::Normal => (self.mode_normal_bg, self.mode_normal_fg, "NORMAL"),
            InputMode::Locked => (self.mode_locked_bg, self.mode_locked_fg, "LOCKED"),
            InputMode::Pane => (self.mode_pane_bg, self.mode_pane_fg, "PANE"),
            InputMode::Tab => (self.mode_tab_bg, self.mode_tab_fg, "TAB"),
            InputMode::Resize => (self.mode_resize_bg, self.mode_resize_fg, "RESIZE"),
            InputMode::Move => (self.mode_move_bg, self.mode_move_fg, "MOVE"),
            InputMode::Scroll => (self.mode_scroll_bg, self.mode_scroll_fg, "SCROLL"),
            InputMode::EnterSearch => (self.mode_entersearch_bg, self.mode_entersearch_fg, "SEARCH"),
            InputMode::Search => (self.mode_search_bg, self.mode_search_fg, "SEARCH"),
            InputMode::RenameTab => (self.mode_renametab_bg, self.mode_renametab_fg, "RENAME"),
            InputMode::RenamePane => (self.mode_renamepane_bg, self.mode_renamepane_fg, "RENAME"),
            InputMode::Session => (self.mode_session_bg, self.mode_session_fg, "SESSION"),
            InputMode::Prompt => (self.mode_prompt_bg, self.mode_prompt_fg, "PROMPT"),
            InputMode::Tmux => (self.mode_tmux_bg, self.mode_tmux_fg, "TMUX"),
        }
    }

    /// Get activity icon color for the given activity.
    pub fn activity_color(&self, activity: &crate::state::Activity) -> Color {
        use crate::state::Activity;
        match activity {
            Activity::Init => self.activity_init_color,
            Activity::Thinking => self.activity_thinking_color,
            Activity::Tool(_) => self.activity_tool_color,
            Activity::Prompting => self.activity_prompt_color,
            Activity::Waiting => self.activity_waiting_color,
            Activity::Notification => self.activity_waiting_color,
            Activity::Done => self.activity_done_color,
            Activity::AgentDone => self.activity_done_color,
            Activity::Idle => self.tab_separator_fg, // dim
        }
    }
}

impl Default for BarConfig {
    fn default() -> Self {
        Self::from_kdl(&BTreeMap::new())
    }
}
