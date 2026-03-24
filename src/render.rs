use crate::config::Color;
use crate::state::{
    unix_now, unix_now_ms, Activity, ClickRegion, MenuAction, MenuClickRegion, SessionInfo,
    SettingKey, State, ViewMode,
};
use std::fmt::Write;
use std::io::Write as IoWrite;
use zellij_tile::prelude::{InputMode, TabInfo};

fn activity_priority(activity: &Activity) -> u8 {
    match activity {
        Activity::Waiting => 8,
        Activity::Tool(_) => 7,
        Activity::Thinking => 6,
        Activity::Prompting => 5,
        Activity::Notification => 4,
        Activity::Init => 3,
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
    if (0x1100..=0x115F).contains(&cp)    // Hangul Jamo
        || (0x2E80..=0x9FFF).contains(&cp) // CJK Radicals..CJK Unified Ideographs
        || (0xAC00..=0xD7AF).contains(&cp) // Hangul Syllables
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0xFE10..=0xFE19).contains(&cp) // Vertical Forms
        || (0xFE30..=0xFE6F).contains(&cp) // CJK Compatibility Forms + Small Form Variants
        || (0xFF01..=0xFF60).contains(&cp) // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&cp) // Fullwidth Signs
        || (0x20000..=0x2FA1F).contains(&cp) // CJK Ext B..Kangxi Supplement
        || (0x30000..=0x323AF).contains(&cp) // CJK Ext G..H
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
                    .map(|&deadline| now_ms < deadline && (now_ms / 250).is_multiple_of(2))
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

/// Render the left prefix (session pill + mode pill + powerline arrows).
/// Returns the number of columns consumed and the optional prefix click region.
fn render_prefix(
    buf: &mut String,
    cols: usize,
    cfg: &crate::config::BarConfig,
    session_text: &str,
    mode_bg: Color,
    mode_fg: Color,
    mode_text: &str,
) -> (usize, Option<(usize, usize)>) {
    let session_pill_text = format!(" {session_text} ");
    let session_pill_width = display_width(&session_pill_text);
    let mode_pill_text = format!(" {mode_text} ");
    let mode_pill_width = display_width(&mode_pill_text);

    let sep_left_width = display_width(&cfg.separator_left);
    let total_prefix_width = session_pill_width + sep_left_width + sep_left_width + mode_pill_width + sep_left_width;

    let mut col = 0usize;
    let mut click_region = None;

    if total_prefix_width <= cols {
        // Session pill (clickable to toggle settings)
        let prefix_start = col;
        write_bg!(buf, cfg.session_bg);
        write_fg!(buf, cfg.session_fg);
        let _ = write!(buf, "{BOLD}{session_pill_text}{RESET}");
        col += session_pill_width;
        click_region = Some((prefix_start, col));

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
        click_region = Some((prefix_start, col));

        write_fg!(buf, cfg.session_bg);
        write_bg!(buf, cfg.bar_bg);
        let _ = write!(buf, "{}", cfg.separator_left);
        col += sep_left_width;
    }

    (col, click_region)
}

/// Fill remaining columns with bar background color.
fn fill_remaining(buf: &mut String, col: usize, cols: usize, bar_bg: Color) {
    if col < cols {
        let remaining = cols - col;
        write_bg!(buf, bar_bg);
        let _ = write!(buf, "{:width$}", "", width = remaining);
    }
    let _ = write!(buf, "{RESET}");
}

/// Minimal rendering for narrow terminals.
/// Progressive display based on available width:
/// - cols < 3: fill with background color only
/// - cols < 10: mode indicator character only (e.g., "N" for Normal)
/// - cols >= 10: mode char + truncated session name
fn render_degraded(
    buf: &mut String,
    cols: usize,
    cfg: &crate::config::BarConfig,
    mode: InputMode,
    session_name: &str,
) {
    let (_mode_bg, _mode_fg, mode_text) = cfg.mode_style(mode);
    let mode_char = mode_text.chars().next().unwrap_or('N');

    write_bg!(buf, cfg.bar_bg);

    if cols < 3 {
        // Absolute minimum: background fill
        let _ = write!(buf, "{:width$}", "", width = cols);
    } else if cols < 10 {
        // Mode indicator only: " N     "
        write_fg!(buf, cfg.session_fg);
        let _ = write!(buf, " {mode_char}");
        let remaining = cols - 2;
        let _ = write!(buf, "{:width$}", "", width = remaining);
    } else {
        // cols >= 10: mode char + truncated session name
        write_fg!(buf, cfg.session_fg);
        let _ = write!(buf, " {mode_char} ");
        let remaining = cols.saturating_sub(4);
        let mut used = 0;
        for c in session_name.chars() {
            let w = char_width(c);
            if used + w > remaining {
                break;
            }
            buf.push(c);
            used += w;
        }
        // Pad remaining
        let pad = remaining.saturating_sub(used);
        if pad > 0 {
            let _ = write!(buf, "{:width$}", "", width = pad);
        }
    }
    let _ = write!(buf, "{RESET}");
}

pub fn render_status_bar(state: &mut State, _rows: usize, cols: usize) {
    state.click_regions.clear();
    state.menu_click_regions.clear();

    let cfg = &state.config;
    let mut buf = String::with_capacity(cols * 4);
    buf.push_str("\x1b[H\x1b[?7l\x1b[?25l");

    if cols < 50 {
        render_degraded(&mut buf, cols, cfg, state.input_mode,
            state.zellij_session_name.as_deref().unwrap_or("zellij"));
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
    let (prefix_cols, click_region) = render_prefix(&mut buf, cols, cfg, session_text, mode_bg, mode_fg, mode_text);
    state.prefix_click_region = click_region;
    let mut col = prefix_cols;

    if col < cols {
        match state.view_mode {
            ViewMode::Normal => render_tabs(state, &mut buf, &mut col, cols),
            ViewMode::Settings => render_settings_menu(state, &mut buf, &mut col, cols),
        }
    }

    fill_remaining(&mut buf, col, cols, state.config.bar_bg);

    print!("{buf}");
    let _ = std::io::stdout().flush();
}

/// Render a single toggle menu item: "● Label" or "○ Label".
/// Returns false if there was not enough space.
#[allow(clippy::too_many_arguments)]
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

/// Render a single tab segment (index + name + indicators + arrows).
/// Returns `(region_start, region_end)` column range for click registration.
#[allow(clippy::too_many_arguments)]
fn render_single_tab(
    buf: &mut String,
    col: &mut usize,
    cols: usize,
    cfg: &crate::config::BarConfig,
    tab: &TabInfo,
    info: &TabRenderInfo,
    max_name_len: usize,
    sep_left_width: usize,
    sep_tab_width: usize,
) -> (usize, usize) {
    let is_active = tab.active;
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
    let char_count = tab.name.chars().count();
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
        for c in tab.name.chars().take(name_chars) {
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

    // Activity indicator
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

    (region_start, *col)
}

struct TabWidthBudget {
    max_name_len: usize,
}

/// Pure computation of tab name width budget. No ANSI output, no side effects.
fn compute_tab_widths(
    tabs: &[&TabInfo],
    tab_infos: &[TabRenderInfo],
    cfg: &crate::config::BarConfig,
    prefix_cols: usize,
    total_cols: usize,
) -> TabWidthBudget {
    let count = tabs.len();
    if count == 0 {
        return TabWidthBudget { max_name_len: 0 };
    }

    let sep_left_width = display_width(&cfg.separator_left);
    let sep_tab_width = display_width(&cfg.separator_tab);

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

    let overhead = prefix_cols + fixed_per_tab + claude_overhead + elapsed_overhead + indicator_overhead + count;
    let max_name_len = if overhead < total_cols {
        ((total_cols - overhead) / count).min(MAX_TAB_NAME_WIDTH)
    } else {
        0
    };

    TabWidthBudget { max_name_len }
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

    let budget = compute_tab_widths(&tabs, &tab_infos, cfg, *col, cols);
    let max_name_len = budget.max_name_len;

    for (i, tab) in tabs.iter().enumerate() {
        if *col + MIN_TAB_COLS > cols {
            break;
        }

        let info = &tab_infos[i];
        let (region_start, region_end) = render_single_tab(
            buf, col, cols, cfg, tab, info, max_name_len, sep_left_width, sep_tab_width,
        );

        state.click_regions.push(ClickRegion {
            start_col: region_start,
            end_col: region_end,
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

    // -- compute_tab_widths --

    #[test]
    fn compute_tab_widths_basic() {
        let tab1 = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab1];
        let infos = vec![TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        }];
        let cfg = crate::config::BarConfig::default();
        let budget = compute_tab_widths(&tabs, &infos, &cfg, 20, 120);
        assert!(budget.max_name_len > 0);
        assert!(budget.max_name_len <= MAX_TAB_NAME_WIDTH);
    }

    #[test]
    fn compute_tab_widths_narrow() {
        let tab1 = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab1];
        let infos = vec![TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        }];
        let cfg = crate::config::BarConfig::default();
        // Very narrow: prefix already consumed most columns
        let budget = compute_tab_widths(&tabs, &infos, &cfg, 90, 100);
        // Should still compute a valid (possibly 0) name length
        assert!(budget.max_name_len <= MAX_TAB_NAME_WIDTH);
    }

    // -- render_degraded --

    #[test]
    fn render_degraded_ultra_narrow() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 2, &cfg, InputMode::Normal, "test");
        // Should contain 2 chars of fill + RESET, no panic
        assert!(buf.contains(RESET));
        assert!(!buf.is_empty());
    }

    #[test]
    fn render_degraded_mode_only() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 8, &cfg, InputMode::Normal, "test-session");
        // Should contain mode char "N"
        assert!(buf.contains('N'));
        assert!(buf.contains(RESET));
    }

    #[test]
    fn render_degraded_with_session_name() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 25, &cfg, InputMode::Normal, "my-project");
        // Should contain mode char and at least part of session name
        assert!(buf.contains('N'));
        assert!(buf.contains("my-project"));
        assert!(buf.contains(RESET));
    }

    #[test]
    fn render_degraded_truncates_long_name() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 12, &cfg, InputMode::Normal, "very-long-session-name");
        // Should contain mode char but truncated name
        assert!(buf.contains('N'));
        // Name should be truncated (12 cols - 4 for " N " = 8 chars max)
        assert!(!buf.contains("very-long-session-name"));
        assert!(buf.contains(RESET));
    }

    // -- render_prefix --

    #[test]
    fn test_render_prefix_contains_session_name() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Normal);
        let mut buf = String::new();
        let (col, click_region) = render_prefix(&mut buf, 120, &cfg, "my-session", mode_bg, mode_fg, mode_text);
        assert!(buf.contains("my-session"));
        assert!(buf.contains("NORMAL"));
        assert!(col > 0);
        assert!(click_region.is_some());
    }

    #[test]
    fn test_render_prefix_returns_click_region() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Normal);
        let mut buf = String::new();
        let (_col, click_region) = render_prefix(&mut buf, 120, &cfg, "test", mode_bg, mode_fg, mode_text);
        let (start, end) = click_region.unwrap();
        assert_eq!(start, 0);
        assert!(end > 0);
    }

    #[test]
    fn test_render_prefix_narrow_skips_mode_pill() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Normal);
        let mut buf = String::new();
        // Total full prefix = " ab "(4) + sep(1) + " NORMAL "(8) + sep(1) + sep(1) = 15
        // Use cols=14 so full prefix doesn't fit, falls back to session-only
        let (_col, _click_region) = render_prefix(&mut buf, 14, &cfg, "ab", mode_bg, mode_fg, mode_text);
        assert!(buf.contains("ab"));
        assert!(!buf.contains("NORMAL"));
    }

    #[test]
    fn test_render_prefix_very_narrow_renders_nothing() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Normal);
        let mut buf = String::new();
        let (col, click_region) = render_prefix(&mut buf, 3, &cfg, "ab", mode_bg, mode_fg, mode_text);
        assert_eq!(col, 0);
        assert!(click_region.is_none());
    }

    #[test]
    fn test_render_prefix_different_modes() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Locked);
        let mut buf = String::new();
        let (_col, _click_region) = render_prefix(&mut buf, 120, &cfg, "test", mode_bg, mode_fg, mode_text);
        assert!(buf.contains("LOCKED"));
    }

    #[test]
    fn test_render_prefix_cjk_session_name() {
        let cfg = crate::config::BarConfig::default();
        let (mode_bg, mode_fg, mode_text) = cfg.mode_style(InputMode::Normal);
        let mut buf = String::new();
        let (col, _click_region) = render_prefix(&mut buf, 120, &cfg, "测试", mode_bg, mode_fg, mode_text);
        assert!(buf.contains("测试"));
        // CJK "测试" = 4 display width, session pill = " 测试 " = 6 display width
        // Full prefix includes session pill + arrows + mode pill + arrow, should be > 6
        assert!(col > 6);
    }

    // -- fill_remaining --

    #[test]
    fn test_fill_remaining_pads_correctly() {
        let mut buf = String::new();
        fill_remaining(&mut buf, 10, 20, (0, 0, 0));
        assert!(buf.contains(RESET));
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_fill_remaining_no_pad_when_full() {
        let mut buf = String::new();
        fill_remaining(&mut buf, 20, 20, (0, 0, 0));
        // Should contain RESET but no space padding
        assert!(buf.contains(RESET));
    }

    // -- compute_tab_info --

    #[test]
    fn test_compute_tab_info_no_sessions() {
        let state = State::default();
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        let result = compute_tab_info(&state, &tabs, 100, 100_000);
        assert!(result[0].best_activity.is_none());
    }

    #[test]
    fn test_compute_tab_info_selects_highest_priority() {
        let mut state = State::default();
        // Pane 1: Thinking (priority 6)
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Thinking,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        // Pane 2: Tool("Bash") (priority 7) — higher than Thinking
        state.sessions.insert(2, SessionInfo {
            session_id: "s2".into(),
            pane_id: 2,
            activity: Activity::Tool("Bash".into()),
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        let result = compute_tab_info(&state, &tabs, 200, 200_000);
        assert_eq!(result[0].best_activity, Some(Activity::Tool("Bash".into())));
    }

    #[test]
    fn test_compute_tab_info_elapsed_shown_after_threshold() {
        let mut state = State::default();
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Thinking,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        // 131 - 100 = 31s >= ELAPSED_THRESHOLD (30)
        let result = compute_tab_info(&state, &tabs, 131, 131_000);
        assert_eq!(result[0].elapsed_str, Some("31s".into()));
    }

    #[test]
    fn test_compute_tab_info_elapsed_hidden_below_threshold() {
        let mut state = State::default();
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Thinking,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        // 129 - 100 = 29s < ELAPSED_THRESHOLD (30)
        let result = compute_tab_info(&state, &tabs, 129, 129_000);
        assert!(result[0].elapsed_str.is_none());
    }

    #[test]
    fn test_compute_tab_info_elapsed_disabled() {
        let mut state = State::default();
        state.settings.elapsed_time = false;
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Thinking,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        let result = compute_tab_info(&state, &tabs, 200, 200_000);
        assert!(result[0].elapsed_str.is_none());
    }

    #[test]
    fn test_compute_tab_info_flash_deadline() {
        let mut state = State::default();
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Done,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        state.flash_deadlines.insert(1, 1000);
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];

        // now_ms=500, before deadline=1000, (500/250) % 2 == 0 → flash bright
        let result = compute_tab_info(&state, &tabs, 0, 500);
        assert!(result[0].is_flash_bright);

        // now_ms=750, before deadline=1000, (750/250) % 2 == 1 → not bright
        let result2 = compute_tab_info(&state, &tabs, 0, 750);
        assert!(!result2[0].is_flash_bright);
    }

    // -- compute_tab_widths (additional) --

    #[test]
    fn test_compute_tab_widths_respects_max_name_width() {
        let tab = TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        };
        let tabs: Vec<&TabInfo> = vec![&tab];
        let infos = vec![TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        }];
        let cfg = crate::config::BarConfig::default();
        // Plenty of columns: budget should be capped at MAX_TAB_NAME_WIDTH
        let budget = compute_tab_widths(&tabs, &infos, &cfg, 10, 200);
        assert_eq!(budget.max_name_len, MAX_TAB_NAME_WIDTH);
    }

    #[test]
    fn test_compute_tab_widths_many_tabs_reduces_budget() {
        let cfg = crate::config::BarConfig::default();
        let make_info = || TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        };

        // 10 tabs
        let tabs_10: Vec<TabInfo> = (0..10).map(|i| TabInfo {
            position: i,
            name: format!("Tab {}", i + 1),
            active: i == 0,
            ..Default::default()
        }).collect();
        let tab_refs_10: Vec<&TabInfo> = tabs_10.iter().collect();
        let infos_10: Vec<TabRenderInfo> = (0..10).map(|_| make_info()).collect();
        let budget_10 = compute_tab_widths(&tab_refs_10, &infos_10, &cfg, 20, 120);

        // 2 tabs
        let tabs_2: Vec<TabInfo> = (0..2).map(|i| TabInfo {
            position: i,
            name: format!("Tab {}", i + 1),
            active: i == 0,
            ..Default::default()
        }).collect();
        let tab_refs_2: Vec<&TabInfo> = tabs_2.iter().collect();
        let infos_2: Vec<TabRenderInfo> = (0..2).map(|_| make_info()).collect();
        let budget_2 = compute_tab_widths(&tab_refs_2, &infos_2, &cfg, 20, 120);

        // Fewer tabs = more width per tab
        assert!(budget_2.max_name_len > budget_10.max_name_len);
    }

    // -- render_single_tab --

    #[test]
    fn test_render_single_tab_active_contains_name() {
        let cfg = crate::config::BarConfig::default();
        let tab = TabInfo {
            position: 0,
            name: "editor".to_string(),
            active: true,
            ..Default::default()
        };
        let info = TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        };
        let sep_left_width = display_width(&cfg.separator_left);
        let sep_tab_width = display_width(&cfg.separator_tab);
        let mut buf = String::new();
        let mut col = 0usize;
        let (region_start, region_end) = render_single_tab(
            &mut buf, &mut col, 120, &cfg, &tab, &info, 10, sep_left_width, sep_tab_width,
        );
        assert!(buf.contains("editor"));
        assert!(region_start < region_end);
    }

    #[test]
    fn test_render_single_tab_with_activity_symbol() {
        let cfg = crate::config::BarConfig::default();
        let tab = TabInfo {
            position: 0,
            name: "main".to_string(),
            active: true,
            ..Default::default()
        };
        let info = TabRenderInfo {
            best_activity: Some(Activity::Tool("Bash".into())),
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        };
        let sep_left_width = display_width(&cfg.separator_left);
        let sep_tab_width = display_width(&cfg.separator_tab);
        let mut buf = String::new();
        let mut col = 0usize;
        render_single_tab(
            &mut buf, &mut col, 120, &cfg, &tab, &info, 10, sep_left_width, sep_tab_width,
        );
        assert!(buf.contains("⚡"));
    }

    #[test]
    fn test_render_single_tab_with_elapsed() {
        let cfg = crate::config::BarConfig::default();
        let tab = TabInfo {
            position: 0,
            name: "work".to_string(),
            active: true,
            ..Default::default()
        };
        let info = TabRenderInfo {
            best_activity: Some(Activity::Thinking),
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: Some("45s".into()),
        };
        let sep_left_width = display_width(&cfg.separator_left);
        let sep_tab_width = display_width(&cfg.separator_tab);
        let mut buf = String::new();
        let mut col = 0usize;
        render_single_tab(
            &mut buf, &mut col, 120, &cfg, &tab, &info, 10, sep_left_width, sep_tab_width,
        );
        assert!(buf.contains("45s"));
    }

    #[test]
    fn test_render_single_tab_truncates_long_name() {
        let cfg = crate::config::BarConfig::default();
        let tab = TabInfo {
            position: 0,
            name: "very-long-tab-name-that-exceeds".to_string(),
            active: true,
            ..Default::default()
        };
        let info = TabRenderInfo {
            best_activity: None,
            is_flash_bright: false,
            waiting_pane_id: None,
            elapsed_str: None,
        };
        let sep_left_width = display_width(&cfg.separator_left);
        let sep_tab_width = display_width(&cfg.separator_tab);
        let mut buf = String::new();
        let mut col = 0usize;
        render_single_tab(
            &mut buf, &mut col, 120, &cfg, &tab, &info, 8, sep_left_width, sep_tab_width,
        );
        // Full name should NOT appear
        assert!(!buf.contains("very-long-tab-name-that-exceeds"));
        // Ellipsis should appear (U+2026)
        assert!(buf.contains("\u{2026}"));
    }

    // -- render_tabs --

    #[test]
    fn test_render_tabs_empty_no_output() {
        let mut state = State::default();
        state.tabs = vec![];
        let mut buf = String::new();
        let mut col = 0usize;
        render_tabs(&mut state, &mut buf, &mut col, 120);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_render_tabs_single_tab() {
        let mut state = State::default();
        state.tabs = vec![TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        }];
        let mut buf = String::new();
        let mut col = 20usize;
        render_tabs(&mut state, &mut buf, &mut col, 120);
        assert!(buf.contains("Tab 1"));
        assert_eq!(state.click_regions.len(), 1);
    }

    #[test]
    fn test_render_tabs_multiple_tabs() {
        let mut state = State::default();
        state.tabs = (0..3).map(|i| TabInfo {
            position: i,
            name: format!("Tab {}", i + 1),
            active: i == 0,
            ..Default::default()
        }).collect();
        let mut buf = String::new();
        let mut col = 20usize;
        render_tabs(&mut state, &mut buf, &mut col, 120);
        assert_eq!(state.click_regions.len(), 3);
    }

    #[test]
    fn test_render_tabs_with_activity() {
        let mut state = State::default();
        state.tabs = vec![TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        }];
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Tool("Bash".into()),
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        let mut buf = String::new();
        let mut col = 20usize;
        render_tabs(&mut state, &mut buf, &mut col, 120);
        assert!(buf.contains("⚡"));
    }

    #[test]
    fn test_render_tabs_narrow_terminal_stops_early() {
        let mut state = State::default();
        state.tabs = (0..5).map(|i| TabInfo {
            position: i,
            name: format!("Tab {}", i + 1),
            active: i == 0,
            ..Default::default()
        }).collect();
        let mut buf = String::new();
        let mut col = 40usize;
        render_tabs(&mut state, &mut buf, &mut col, 60);
        // With col=40, cols=60, only 20 cols remaining. Not all 5 tabs can fit.
        assert!(state.click_regions.len() < 5);
    }

    // -- render_status_bar --

    #[test]
    fn test_render_status_bar_no_panic_default_state() {
        let mut state = State::default();
        render_status_bar(&mut state, 1, 120);
        // Default state has no tabs, so click_regions should be empty
        assert!(state.click_regions.is_empty());
    }

    #[test]
    fn test_render_status_bar_with_tabs_populates_click_regions() {
        let mut state = State::default();
        state.tabs = vec![TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        }];
        state.sessions.insert(1, SessionInfo {
            session_id: "s1".into(),
            pane_id: 1,
            activity: Activity::Thinking,
            tab_name: Some("Tab 1".into()),
            tab_index: Some(0),
            last_event_ts: 100,
            cwd: None,
        });
        render_status_bar(&mut state, 1, 120);
        assert!(state.click_regions.len() > 0);
    }

    #[test]
    fn test_render_status_bar_narrow_triggers_degraded() {
        let mut state = State::default();
        state.zellij_session_name = Some("test".into());
        state.tabs = vec![TabInfo {
            position: 0,
            name: "Tab 1".to_string(),
            active: true,
            ..Default::default()
        }];
        render_status_bar(&mut state, 1, 30);
        // Degraded mode doesn't register tab click regions
        assert!(state.click_regions.is_empty());
    }

    // -- render_menu_item --

    #[test]
    fn test_render_menu_item_on_state() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        let mut col = 0usize;
        let mut regions: Vec<MenuClickRegion> = Vec::new();
        let result = render_menu_item(
            &mut buf, &mut col, 120, cfg.bar_bg,
            true, "Flash: brief", true,
            &mut regions,
            MenuAction::ToggleSetting(SettingKey::Flash),
            cfg.menu_active_sym, cfg.menu_inactive_sym,
            cfg.menu_active_label, cfg.menu_dim_label,
        );
        assert!(result);
        assert!(buf.contains("●"));
        assert!(buf.contains("Flash: brief"));
        assert_eq!(regions.len(), 1);
    }

    #[test]
    fn test_render_menu_item_off_state() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        let mut col = 0usize;
        let mut regions: Vec<MenuClickRegion> = Vec::new();
        let result = render_menu_item(
            &mut buf, &mut col, 120, cfg.bar_bg,
            false, "Flash: off", true,
            &mut regions,
            MenuAction::ToggleSetting(SettingKey::Flash),
            cfg.menu_active_sym, cfg.menu_inactive_sym,
            cfg.menu_active_label, cfg.menu_dim_label,
        );
        assert!(result);
        assert!(buf.contains("○"));
    }

    #[test]
    fn test_render_menu_item_not_enough_space() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        let mut col = 118usize;
        let mut regions: Vec<MenuClickRegion> = Vec::new();
        // is_first=false, so spacing check: col(118) + MIN_MENU_ITEM_COLS(4) >= cols(120)
        let result = render_menu_item(
            &mut buf, &mut col, 120, cfg.bar_bg,
            true, "Test", false,
            &mut regions,
            MenuAction::ToggleSetting(SettingKey::Flash),
            cfg.menu_active_sym, cfg.menu_inactive_sym,
            cfg.menu_active_label, cfg.menu_dim_label,
        );
        assert!(!result);
    }

    // -- render_settings_menu --

    #[test]
    fn test_render_settings_menu_shows_all_items() {
        let mut state = State::default();
        let mut buf = String::new();
        let mut col = 0usize;
        render_settings_menu(&mut state, &mut buf, &mut col, 120);
        assert!(buf.contains("Flash:"));
        assert!(buf.contains("Elapsed:"));
        assert!(buf.contains("Notify:"));
        assert!(buf.contains("×"));
        // 3 settings toggles + 1 close button
        assert_eq!(state.menu_click_regions.len(), 4);
    }

    #[test]
    fn test_render_settings_menu_narrow_truncates() {
        let mut state = State::default();
        let mut buf = String::new();
        let mut col = 20usize;
        render_settings_menu(&mut state, &mut buf, &mut col, 40);
        // Not all items fit in 20 remaining cols
        assert!(state.menu_click_regions.len() < 4);
    }

    // -- render_degraded (additional) --

    #[test]
    fn test_render_degraded_contains_session_name_directly() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 30, &cfg, InputMode::Normal, "work-project");
        assert!(buf.contains("work-project"));
        assert!(buf.contains('N'));
    }

    #[test]
    fn test_render_degraded_locked_mode_indicator() {
        let cfg = crate::config::BarConfig::default();
        let mut buf = String::new();
        render_degraded(&mut buf, 15, &cfg, InputMode::Locked, "test");
        assert!(buf.contains('L'));
    }
}
