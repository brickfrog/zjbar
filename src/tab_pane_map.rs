use std::collections::HashMap;
use zellij_tile::prelude::*;

/// Build a mapping from terminal pane_id -> (tab_index, tab_name).
/// Uses PaneManifest (keyed by tab_index) cross-referenced with TabInfo list.
pub fn build_pane_to_tab_map(
    tabs: &[TabInfo],
    manifest: &PaneManifest,
) -> HashMap<u32, (usize, String)> {
    let tab_name_by_position: HashMap<usize, String> =
        tabs.iter().map(|t| (t.position, t.name.clone())).collect();

    let mut map = HashMap::new();
    for (&tab_index, panes) in &manifest.panes {
        let tab_name = tab_name_by_position
            .get(&tab_index)
            .cloned()
            .unwrap_or_default();
        for pane in panes {
            if !pane.is_plugin {
                map.insert(pane.id, (tab_index, tab_name.clone()));
            }
        }
    }
    map
}

pub fn build_pane_title_map(manifest: &PaneManifest) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    for panes in manifest.panes.values() {
        for pane in panes {
            if !pane.is_plugin {
                map.insert(pane.id, pane.title.clone());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tab(position: usize, name: &str) -> TabInfo {
        TabInfo {
            position,
            name: name.to_string(),
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

    #[test]
    fn empty_inputs() {
        let map = build_pane_to_tab_map(&[], &make_manifest(vec![]));
        assert!(map.is_empty());
    }

    #[test]
    fn single_tab_single_terminal_pane() {
        let tabs = [make_tab(0, "main")];
        let manifest = make_manifest(vec![(0, vec![make_pane(1, false)])]);
        let map = build_pane_to_tab_map(&tabs, &manifest);
        assert_eq!(map.len(), 1);
        assert_eq!(map[&1], (0, "main".to_string()));
    }

    #[test]
    fn plugin_panes_excluded() {
        let tabs = [make_tab(0, "tab0")];
        let manifest = make_manifest(vec![(
            0,
            vec![make_pane(1, false), make_pane(2, true), make_pane(3, false)],
        )]);
        let map = build_pane_to_tab_map(&tabs, &manifest);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&1));
        assert!(!map.contains_key(&2)); // plugin pane excluded
        assert!(map.contains_key(&3));
    }

    #[test]
    fn multiple_tabs() {
        let tabs = [make_tab(0, "alpha"), make_tab(1, "beta")];
        let manifest = make_manifest(vec![
            (0, vec![make_pane(10, false)]),
            (1, vec![make_pane(20, false), make_pane(21, false)]),
        ]);
        let map = build_pane_to_tab_map(&tabs, &manifest);
        assert_eq!(map.len(), 3);
        assert_eq!(map[&10], (0, "alpha".to_string()));
        assert_eq!(map[&20], (1, "beta".to_string()));
        assert_eq!(map[&21], (1, "beta".to_string()));
    }

    #[test]
    fn manifest_tab_not_in_tabs_uses_empty_name() {
        // manifest references tab_index=5 but no TabInfo has position=5
        let tabs = [make_tab(0, "only-tab")];
        let manifest = make_manifest(vec![(5, vec![make_pane(99, false)])]);
        let map = build_pane_to_tab_map(&tabs, &manifest);
        assert_eq!(map[&99], (5, String::new()));
    }

    #[test]
    fn all_plugin_panes_yields_empty_map() {
        let tabs = [make_tab(0, "tab0")];
        let manifest = make_manifest(vec![(0, vec![make_pane(1, true), make_pane(2, true)])]);
        let map = build_pane_to_tab_map(&tabs, &manifest);
        assert!(map.is_empty());
    }

    #[test]
    fn pane_title_map_excludes_plugin_panes() {
        let manifest = make_manifest(vec![(
            0,
            vec![
                PaneInfo {
                    id: 1,
                    title: "leaf work".to_string(),
                    ..Default::default()
                },
                PaneInfo {
                    id: 2,
                    title: "zjbar".to_string(),
                    is_plugin: true,
                    ..Default::default()
                },
            ],
        )]);

        let map = build_pane_title_map(&manifest);
        assert_eq!(map.len(), 1);
        assert_eq!(map[&1], "leaf work".to_string());
        assert!(!map.contains_key(&2));
    }
}
