use crate::config::Color;
use crate::state::{
    unix_now, unix_now_ms, Activity, ClickRegion, MenuAction, MenuClickRegion, SessionInfo,
    SettingKey, State, ViewMode,
};
use std::fmt::Write;
use std::io::Write as IoWrite;
use zellij_tile::prelude::TabInfo;

fn activity_priority(activity: &Activity) -> u8 {
    match activity {
        Activity::Waiting => 8,
        Activity::Tool(_) => 7,
        Activity::Thinking => 6,
        Activity::Prompting => 5,
        Activity::Notification => 4,
        Activity::Init => 3,
        Activity::Suspending => 2,
        Activity::Done => 2,
        Activity::AgentDone => 1,
        Activity::Idle => 0,
    }
}

fn activity_symbol(activity: &Activity) -> &'static str {
    match activity {
        Activity::Init => "◆",
        Activity::Thinking => "●",
        Activity::Tool(name) => match name.as_str() {
            "Bash" => "⚡",
            "Read" | "Glob" | "Grep" => "◉",
            "Edit" | "Write" => "✎",
            "Task" => "⊜",
            "WebSearch" | "WebFetch" => "◈",
            _ => "⚙",
        },
        Activity::Prompting => "▶",
        Activity::Waiting => "⚠",
        Activity::Notification => "◇",
        Activity::Suspending => "⏳",
        Activity::Done | Activity::AgentDone => "✓",
        Activity::Idle => "○",
    }
}

macro_rules! write_fg {
    ($buf:expr, $r:expr, $g:expr, $b:expr) => {
        let _ = write!($buf, "\x1b[38;2;{};{};{}m", $r, $g, $b);
    };
    ($buf:expr, $c:expr) => {
        let _ = write!($buf, "\x1b[38;2;{};{};{}m", $c.0, $c.1, $c.2);
    };
}

macro_rules! write_bg {
    ($buf:expr, $c:expr) => {
        let _ = write!($buf, "\x1b[48;2;{};{};{}m", $c.0, $c.1, $c.2);
    };
}

fn char_width(c: char) -> usize {
    let cp = c as u32;
    if (0x2E80..=0x9FFF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF01..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x2FA1F).contains(&cp)
        || (0x30000..=0x323AF).contains(&cp)
    {
        2
    } else {
        1
    }
}

fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Number of decimal digits in a positive integer (e.g. 1→1, 10→2, 100→3).
fn digit_count(n: usize) -> usize {
    if n == 0 { return 1; }
    let mut d = 0;
    let mut v = n;
    while v > 0 {
        d += 1;
        v /= 10;
    }
    d
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const ELAPSED_THRESHOLD: u64 = 30;

/// Maximum display width for a tab name before truncation.
const MAX_TAB_NAME_WIDTH: usize = 20;
/// Minimum columns required to start rendering a new tab.
const MIN_TAB_COLS: usize = 8;
/// Minimum remaining columns to render the next settings menu item.
const MIN_MENU_ITEM_COLS: usize = 4;
/// Minimum remaining columns to render the close button.
const MIN_CLOSE_BTN_COLS: usize = 3;

fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

struct TabRenderInfo {
    best_activity: Option<Activity>,
    is_flash_bright: bool,
    waiting_pane_id: Option<u32>,
    elapsed_str: Option<String>,
}

fn compute_tab_info(
    state: &State,
    tabs: &[&TabInfo],
    now_s: u64,
    now_ms: u64,
) -> Vec<TabRenderInfo> {
    tabs.iter()
        .map(|tab| {
            let tab_sessions: Vec<&SessionInfo> = state
                .sessions
                .values()
                .filter(|s| s.tab_index == Some(tab.position))
                .collect();

            let best_session = tab_sessions
                .iter()
                .copied()
                .max_by_key(|s| activity_priority(&s.activity));

            let is_flash_bright = tab_sessions.iter().any(|s| {
                state
                    .flash_deadlines
                    .get(&s.pane_id)
                    .map(|&deadline| now_ms < deadline && (now_ms / 250) % 2 == 0)
                    .unwrap_or(false)
            });

            let waiting_pane_id = tab_sessions
                .iter()
                .find(|s| matches!(s.activity, Activity::Waiting))
                .map(|s| s.pane_id);

            let elapsed_str = if !state.settings.elapsed_time {
                None
            } else {
                best_session.and_then(|s| {
                    let elapsed = now_s.saturating_sub(s.last_event_ts);
                    if elapsed >= ELAPSED_THRESHOLD {
                        Some(format_elapsed(elapsed))
                    } else {
                        None
                    }
                })
            };

            TabRenderInfo {
                best_activity: best_session.map(|s| s.activity.clone()),
                is_flash_bright,
                waiting_pane_id,
                elapsed_str,
            }
        })
        .collect()
}

pub fn render_status_bar(state: &mut State, _rows: usize, cols: usize) {
    state.click_regions.clear();
    state.menu_click_regions.clear();

    let cfg = &state.config;
    let mut buf = String::with_capacity(cols * 4);
    buf.push_str("\x1b[H\x1b[?7l\x1b[?25l");

    if cols < 5 {
        write_bg!(buf, cfg.bar_bg);
        let _ = write!(buf, "{:width$}{RESET}", "", width = cols);
        print!("{buf}");
        let _ = std::io::stdout().flush();
        return;
    }

    // === Left prefix: [session pill][arrow][mode pill][arrow] ===
    let (mode_bg, mode_fg, mode_text) = cfg.mode_style(state.input_mode);
    let session_text = state
        .zellij_session_name
        .as_deref()
        .unwrap_or("zellij");

    let session_pill_text = format!(" {session_text} ");
    let session_pill_width = display_width(&session_pill_text);
    let mode_pill_text = format!(" {mode_text} ");
    let mode_pill_width = display_width(&mode_pill_text);

    let sep_left_width = display_width(&cfg.separator_left);
    let total_prefix_width = session_pill_width + sep_left_width + sep_left_width + mode_pill_width + sep_left_width;

    let mut col = 0usize;
    if total_prefix_width <= cols {
        // Session pill (clickable to toggle settings)
        let prefix_start = col;
        write_bg!(buf, cfg.session_bg);
        write_fg!(buf, cfg.session_fg);
        let _ = write!(buf, "{BOLD}{session_pill_text}{RESET}");
        col += session_pill_width;
        state.prefix_click_region = Some((prefix_start, col));

        // Arrow: session → mode
        write_fg!(buf, cfg.session_bg);
        write_bg!(buf, mode_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        col += sep_left_width;

        // Mode pill
        write_bg!(buf, mode_bg);
        write_fg!(buf, mode_fg);
        let _ = write!(buf, "{BOLD}{mode_pill_text}{RESET}");
        col += mode_pill_width;

        // Arrow: mode → bar_bg
        write_fg!(buf, mode_bg);
        write_bg!(buf, cfg.bar_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        col += sep_left_width;
    } else if session_pill_width + sep_left_width <= cols {
        let prefix_start = col;
        write_bg!(buf, cfg.session_bg);
        write_fg!(buf, cfg.session_fg);
        let _ = write!(buf, "{BOLD}{session_pill_text}{RESET}");
        col += session_pill_width;
        state.prefix_click_region = Some((prefix_start, col));

        write_fg!(buf, cfg.session_bg);
        write_bg!(buf, cfg.bar_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        col += sep_left_width;
    }

    if col < cols {
        match state.view_mode {
            ViewMode::Normal => render_tabs(state, &mut buf, &mut col, cols),
            ViewMode::Settings => render_settings_menu(state, &mut buf, &mut col, cols),
        }
    }

    // Fill remaining with bar bg
    if col < cols {
        let remaining = cols - col;
        write_bg!(buf, state.config.bar_bg);
        let _ = write!(buf, "{:width$}", "", width = remaining);
    }
    let _ = write!(buf, "{RESET}");

    print!("{buf}");
    let _ = std::io::stdout().flush();
}

/// Render a single toggle menu item: "● Label" or "○ Label".
/// Returns false if there was not enough space.
fn render_menu_item(
    buf: &mut String,
    col: &mut usize,
    cols: usize,
    bar_bg: Color,
    is_on: bool,
    label: &str,
    is_first: bool,
    regions: &mut Vec<MenuClickRegion>,
    action: MenuAction,
    active_sym: Color,
    inactive_sym: Color,
    active_label: Color,
    dim_label: Color,
) -> bool {
    // Spacing between items (skip for the first item)
    if !is_first {
        if *col + MIN_MENU_ITEM_COLS >= cols {
            return false;
        }
        write_bg!(buf, bar_bg);
        let _ = write!(buf, "  ");
        *col += 2;
    }

    let (sym, sym_c, label_c) = if is_on {
        ("●", active_sym, active_label)
    } else {
        ("○", inactive_sym, dim_label)
    };

    let start = *col;
    write_bg!(buf, bar_bg);
    write_fg!(buf, sym_c);
    let _ = write!(buf, "{sym} ");
    write_fg!(buf, label_c);
    let _ = write!(buf, "{label}");
    *col += 2 + label.len();

    regions.push(MenuClickRegion {
        start_col: start,
        end_col: *col,
        action,
    });
    true
}

fn render_settings_menu(
    state: &mut State,
    buf: &mut String,
    col: &mut usize,
    cols: usize,
) {
    // Spacing after prefix
    write_bg!(buf, state.config.bar_bg);
    let _ = write!(buf, " ");
    *col += 1;

    let bar_bg = state.config.bar_bg;
    let cfg = &state.config;

    // --- Flash ---
    let flash_label = format!("Flash: {}", state.settings.flash.label());
    if !render_menu_item(
        buf, col, cols, bar_bg,
        state.settings.flash != crate::state::FlashMode::Off,
        &flash_label,
        true,
        &mut state.menu_click_regions,
        MenuAction::ToggleSetting(SettingKey::Flash),
        cfg.menu_active_sym, cfg.menu_inactive_sym,
        cfg.menu_active_label, cfg.menu_dim_label,
    ) { return; }

    // --- Elapsed time ---
    let elapsed_label = if state.settings.elapsed_time { "Elapsed: on" } else { "Elapsed: off" };
    if !render_menu_item(
        buf, col, cols, bar_bg,
        state.settings.elapsed_time,
        elapsed_label,
        false,
        &mut state.menu_click_regions,
        MenuAction::ToggleSetting(SettingKey::ElapsedTime),
        cfg.menu_active_sym, cfg.menu_inactive_sym,
        cfg.menu_active_label, cfg.menu_dim_label,
    ) { return; }

    // --- Notifications ---
    let notify_label = format!("Notify: {}", state.settings.notifications.label());
    if !render_menu_item(
        buf, col, cols, bar_bg,
        state.settings.notifications != crate::state::NotifyMode::Off,
        &notify_label,
        false,
        &mut state.menu_click_regions,
        MenuAction::ToggleSetting(SettingKey::Notifications),
        cfg.menu_active_sym, cfg.menu_inactive_sym,
        cfg.menu_active_label, cfg.menu_dim_label,
    ) { return; }

    // --- Close button ---
    if *col + MIN_CLOSE_BTN_COLS >= cols { return; }
    write_bg!(buf, bar_bg);
    let _ = write!(buf, "  ");
    *col += 2;
    let start = *col;
    write_bg!(buf, bar_bg);
    write_fg!(buf, cfg.menu_close);
    let _ = write!(buf, "×");
    *col += 1;
    state.menu_click_regions.push(MenuClickRegion {
        start_col: start,
        end_col: *col,
        action: MenuAction::CloseMenu,
    });
}

fn render_tabs(
    state: &mut State,
    buf: &mut String,
    col: &mut usize,
    cols: usize,
) {
    let cfg = &state.config;
    let now_s = unix_now();
    let now_ms = unix_now_ms();

    let mut tabs: Vec<&TabInfo> = state.tabs.iter().collect();
    tabs.sort_by_key(|t| t.position);

    let count = tabs.len();
    if count == 0 {
        return;
    }

    let sep_left_width = display_width(&cfg.separator_left);
    let sep_tab_width = display_width(&cfg.separator_tab);

    let tab_infos = compute_tab_info(state, &tabs, now_s, now_ms);

    // Compute max tab name length
    let fixed_per_tab: usize = tabs.iter().map(|t| {
        let idx_digits = digit_count(t.position + 1);
        let mid_sep = if t.active { sep_left_width } else { sep_tab_width };
        // leading_sep + space + index + space + mid_sep + trailing_space + trailing_sep
        sep_left_width + 1 + idx_digits + 1 + mid_sep + 1 + sep_left_width
    }).sum();
    let claude_overhead: usize = tab_infos
        .iter()
        .map(|info| if info.best_activity.is_some() { 2 } else { 0 })
        .sum();
    let elapsed_overhead: usize = tab_infos
        .iter()
        .map(|info| info.elapsed_str.as_ref().map_or(0, |s| 1 + s.len()))
        .sum();

    let indicator_overhead: usize = tabs.iter().map(|t| {
        let mut w = 0;
        if t.is_fullscreen_active { w += display_width(&cfg.tab_fullscreen_indicator); }
        if t.are_floating_panes_visible { w += display_width(&cfg.tab_floating_indicator); }
        w
    }).sum();

    let overhead = *col + fixed_per_tab + claude_overhead + elapsed_overhead + indicator_overhead + count;
    let max_name_len = if overhead < cols {
        ((cols - overhead) / count).min(MAX_TAB_NAME_WIDTH)
    } else {
        0
    };

    for (i, tab) in tabs.iter().enumerate() {
        if *col + MIN_TAB_COLS > cols {
            break;
        }

        let info = &tab_infos[i];
        let is_active = tab.active;
        let tab_name = &tab.name;

        let is_flash_bright = info.is_flash_bright;

        // Tab colors
        let tab_bg = if is_flash_bright {
            cfg.flash_bg
        } else if is_active {
            cfg.tab_active_bg
        } else {
            cfg.tab_inactive_bg
        };

        let tab_fg = if is_flash_bright {
            cfg.flash_fg
        } else if is_active {
            cfg.tab_active_fg
        } else {
            cfg.tab_inactive_fg
        };

        // Compute name truncation parameters (no allocation)
        let char_count = tab_name.chars().count();
        let (name_chars, needs_ellipsis) = if max_name_len == 0 {
            (0, false)
        } else if char_count > max_name_len {
            (max_name_len.saturating_sub(1), true)
        } else {
            (char_count, false)
        };

        let region_start = *col;

        // Determine index colors (active tabs get a highlighted index)
        let (idx_bg, idx_fg) = if is_flash_bright {
            (cfg.flash_bg, cfg.flash_fg)
        } else if is_active {
            (cfg.tab_active_index_bg, cfg.tab_active_index_fg)
        } else {
            (tab_bg, tab_fg)
        };

        // Leading arrow: [bar_bg → idx_bg]
        write_fg!(buf, cfg.bar_bg);
        write_bg!(buf, idx_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        *col += sep_left_width;

        // Index part: " N "
        let idx_num = tab.position + 1;
        write_bg!(buf, idx_bg);
        write_fg!(buf, idx_fg);
        let _ = write!(buf, "{BOLD} {idx_num} {RESET}");
        *col += 1 + digit_count(idx_num) + 1;

        // Transition from index to name area
        if is_active && !is_flash_bright {
            // Powerline arrow: idx_bg → tab_bg
            write_fg!(buf, idx_bg);
            write_bg!(buf, tab_bg);
            let _ = write!(buf, "{}", cfg.separator_left);
            *col += sep_left_width;
        } else {
            // Thin separator (same bg)
            write_bg!(buf, tab_bg);
            write_fg!(buf, cfg.tab_separator_fg);
            let _ = write!(buf, "{}", cfg.separator_tab);
            *col += sep_tab_width;
        }

        // Name part: " name "
        write_bg!(buf, tab_bg);
        write_fg!(buf, tab_fg);
        let _ = write!(buf, "{BOLD} ");
        *col += 1;

        if name_chars > 0 {
            for c in tab_name.chars().take(name_chars) {
                buf.push(c);
                *col += char_width(c);
            }
            if needs_ellipsis {
                buf.push('\u{2026}');
                *col += 1;
            }
        }

        // Fullscreen / floating indicators
        if tab.is_fullscreen_active {
            let ind = &cfg.tab_fullscreen_indicator;
            let _ = write!(buf, "{ind}");
            *col += display_width(ind);
        }
        if tab.are_floating_panes_visible {
            let ind = &cfg.tab_floating_indicator;
            let _ = write!(buf, "{ind}");
            *col += display_width(ind);
        }

        // Claude activity indicator
        if let Some(ref activity) = info.best_activity {
            if !matches!(activity, Activity::Idle) {
                let symbol = activity_symbol(activity);
                let icon_color = if is_flash_bright {
                    cfg.flash_fg
                } else {
                    cfg.activity_color(activity)
                };
                let _ = write!(buf, " {RESET}");
                write_bg!(buf, tab_bg);
                write_fg!(buf, icon_color);
                let _ = write!(buf, "{symbol}");
                *col += 1 + display_width(symbol);
            }

            if let Some(ref es) = info.elapsed_str {
                if *col + 1 + es.len() + 1 < cols {
                    let _ = write!(buf, " {RESET}");
                    write_bg!(buf, tab_bg);
                    write_fg!(buf, cfg.elapsed_fg);
                    let _ = write!(buf, "{es}");
                    *col += 1 + es.len();
                }
            }
        }

        // Trailing space + closing arrow
        let _ = write!(buf, "{RESET}");
        write_bg!(buf, tab_bg);
        let _ = write!(buf, " ");
        *col += 1;
        write_fg!(buf, tab_bg);
        write_bg!(buf, cfg.bar_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        *col += sep_left_width;

        // Register click region
        state.click_regions.push(ClickRegion {
            start_col: region_start,
            end_col: *col,
            tab_index: tab.position,
            pane_id: info.waiting_pane_id.unwrap_or(0),
            is_waiting: info.waiting_pane_id.is_some(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- char_width --

    #[test]
    fn char_width_ascii() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('Z'), 1);
        assert_eq!(char_width('0'), 1);
        assert_eq!(char_width(' '), 1);
        assert_eq!(char_width('!'), 1);
    }

    #[test]
    fn char_width_cjk() {
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('文'), 2);
        assert_eq!(char_width('日'), 2);
        assert_eq!(char_width('한'), 2); // Korean
    }

    #[test]
    fn char_width_fullwidth() {
        assert_eq!(char_width('Ａ'), 2); // fullwidth A (U+FF21)
        assert_eq!(char_width('１'), 2); // fullwidth 1 (U+FF11)
    }

    // -- display_width --

    #[test]
    fn display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn display_width_mixed() {
        assert_eq!(display_width("Hello世界"), 9); // 5 + 2*2
        assert_eq!(display_width("ab中cd"), 6); // 2 + 2 + 2
    }

    // -- digit_count --

    #[test]
    fn digit_count_values() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(1), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
        assert_eq!(digit_count(999), 3);
        assert_eq!(digit_count(1000), 4);
    }

    // -- format_elapsed --

    #[test]
    fn format_elapsed_seconds() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(30), "30s");
        assert_eq!(format_elapsed(59), "59s");
    }

    #[test]
    fn format_elapsed_minutes() {
        assert_eq!(format_elapsed(60), "1m");
        assert_eq!(format_elapsed(120), "2m");
        assert_eq!(format_elapsed(3599), "59m");
    }

    #[test]
    fn format_elapsed_hours() {
        assert_eq!(format_elapsed(3600), "1h");
        assert_eq!(format_elapsed(7200), "2h");
    }

    // -- activity_priority --

    #[test]
    fn activity_priority_ordering() {
        assert!(activity_priority(&Activity::Waiting) > activity_priority(&Activity::Tool("Bash".into())));
        assert!(activity_priority(&Activity::Tool("Bash".into())) > activity_priority(&Activity::Thinking));
        assert!(activity_priority(&Activity::Thinking) > activity_priority(&Activity::Prompting));
        assert!(activity_priority(&Activity::Done) > activity_priority(&Activity::AgentDone));
        assert!(activity_priority(&Activity::AgentDone) > activity_priority(&Activity::Idle));
    }

    // -- activity_symbol --

    #[test]
    fn activity_symbol_tools() {
        assert_eq!(activity_symbol(&Activity::Tool("Bash".into())), "⚡");
        assert_eq!(activity_symbol(&Activity::Tool("Read".into())), "◉");
        assert_eq!(activity_symbol(&Activity::Tool("Edit".into())), "✎");
        assert_eq!(activity_symbol(&Activity::Tool("Unknown".into())), "⚙");
    }

    #[test]
    fn activity_symbol_states() {
        assert_eq!(activity_symbol(&Activity::Thinking), "●");
        assert_eq!(activity_symbol(&Activity::Done), "✓");
        assert_eq!(activity_symbol(&Activity::Waiting), "⚠");
        assert_eq!(activity_symbol(&Activity::Idle), "○");
    }
}
