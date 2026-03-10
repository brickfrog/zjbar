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

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const ELAPSED_THRESHOLD: u64 = 30;

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

    // Colors as tuples (zero-allocation)
    let active_sym_c: Color = (80, 200, 120);
    let inactive_sym_c: Color = (100, 100, 100);
    let active_label_c: Color = (192, 202, 245);
    let dim_label_c: Color = (100, 100, 100);

    // --- Flash ---
    {
        let is_on = state.settings.flash != crate::state::FlashMode::Off;
        let (sym, sym_c, label_c) = if is_on {
            ("●", &active_sym_c, &active_label_c)
        } else {
            ("○", &inactive_sym_c, &dim_label_c)
        };
        let label = format!("Flash: {}", state.settings.flash.label());
        let start = *col;
        write_bg!(buf, state.config.bar_bg);
        write_fg!(buf, *sym_c);
        let _ = write!(buf, "{sym} ");
        write_fg!(buf, *label_c);
        let _ = write!(buf, "{label}");
        *col += 2 + label.len();

        state.menu_click_regions.push(MenuClickRegion {
            start_col: start,
            end_col: *col,
            action: MenuAction::ToggleSetting(SettingKey::Flash),
        });
    }

    if *col + 4 >= cols { return; }

    // --- Elapsed time ---
    {
        write_bg!(buf, state.config.bar_bg);
        let _ = write!(buf, "  ");
        *col += 2;

        let on = state.settings.elapsed_time;
        let (sym, sym_c, label_c) = if on {
            ("●", &active_sym_c, &active_label_c)
        } else {
            ("○", &inactive_sym_c, &dim_label_c)
        };
        let label = if on { "Elapsed: on" } else { "Elapsed: off" };
        let start = *col;
        write_bg!(buf, state.config.bar_bg);
        write_fg!(buf, *sym_c);
        let _ = write!(buf, "{sym} ");
        write_fg!(buf, *label_c);
        let _ = write!(buf, "{label}");
        *col += 2 + label.len();

        state.menu_click_regions.push(MenuClickRegion {
            start_col: start,
            end_col: *col,
            action: MenuAction::ToggleSetting(SettingKey::ElapsedTime),
        });
    }

    if *col + 4 >= cols { return; }

    // --- Notifications ---
    {
        write_bg!(buf, state.config.bar_bg);
        let _ = write!(buf, "  ");
        *col += 2;

        let is_on = state.settings.notifications != crate::state::NotifyMode::Off;
        let (sym, sym_c, label_c) = if is_on {
            ("●", &active_sym_c, &active_label_c)
        } else {
            ("○", &inactive_sym_c, &dim_label_c)
        };
        let label = format!("Notify: {}", state.settings.notifications.label());
        let start = *col;
        write_bg!(buf, state.config.bar_bg);
        write_fg!(buf, *sym_c);
        let _ = write!(buf, "{sym} ");
        write_fg!(buf, *label_c);
        let _ = write!(buf, "{label}");
        *col += 2 + label.len();

        state.menu_click_regions.push(MenuClickRegion {
            start_col: start,
            end_col: *col,
            action: MenuAction::ToggleSetting(SettingKey::Notifications),
        });
    }

    if *col + 3 >= cols { return; }

    // --- Close button ---
    {
        write_bg!(buf, state.config.bar_bg);
        let _ = write!(buf, "  ");
        *col += 2;
        let start = *col;
        write_bg!(buf, state.config.bar_bg);
        write_fg!(buf, 255u8, 60u8, 60u8);
        let _ = write!(buf, "×");
        *col += 1;

        state.menu_click_regions.push(MenuClickRegion {
            start_col: start,
            end_col: *col,
            action: MenuAction::CloseMenu,
        });
    }
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
        let idx_str = format!("{}", t.position + 1);
        let mid_sep = if t.active { sep_left_width } else { sep_tab_width };
        // leading_sep + space + index + space + mid_sep + trailing_space + trailing_sep
        sep_left_width + 1 + idx_str.len() + 1 + mid_sep + 1 + sep_left_width
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
        ((cols - overhead) / count).min(20)
    } else {
        0
    };

    for (i, tab) in tabs.iter().enumerate() {
        if *col + 8 > cols {
            break;
        }

        let info = &tab_infos[i];
        let is_claude = info.best_activity.is_some();
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

        // Truncate name
        let char_count = tab_name.chars().count();
        let truncated = if max_name_len == 0 {
            String::new()
        } else if char_count > max_name_len {
            let s: String = tab_name.chars().take(max_name_len.saturating_sub(1)).collect();
            format!("{s}…")
        } else {
            tab_name.to_string()
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
        let idx_str = format!("{}", tab.position + 1);
        write_bg!(buf, idx_bg);
        write_fg!(buf, idx_fg);
        let _ = write!(buf, "{BOLD} {idx_str} {RESET}");
        *col += 1 + idx_str.len() + 1;

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

        if !truncated.is_empty() {
            let _ = write!(buf, "{truncated}");
            *col += display_width(&truncated);
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
        if is_claude {
            let activity = info.best_activity.as_ref().unwrap();
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
        if is_claude {
            state.click_regions.push(ClickRegion {
                start_col: region_start,
                end_col: *col,
                tab_index: tab.position,
                pane_id: info.waiting_pane_id.unwrap_or(0),
                is_waiting: info.waiting_pane_id.is_some(),
            });
        } else {
            state.click_regions.push(ClickRegion {
                start_col: region_start,
                end_col: *col,
                tab_index: tab.position,
                pane_id: 0,
                is_waiting: false,
            });
        }
    }
}
