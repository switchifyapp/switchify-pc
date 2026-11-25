//! The built-in menus as panel documents. Home and the action menus are
//! expressed in the same shape as the user's own panels, so the runtime
//! builds them from data and the user can rearrange them and reset them.
//! The point-scan context pages and the power confirmation pages stay fixed.
use crate::{
    custom_panels::{Cell, CustomPanel},
    panel_actions::{BuiltInPanel, PanelAction},
    scan_menu::{Item, Kind},
};
use std::collections::BTreeMap;

/// The menus a user may rearrange, in the order the Panels page lists them.
pub const EDITABLE: [Kind; 10] = [
    Kind::Home,
    Kind::Mouse,
    Kind::Editing,
    Kind::Windows,
    Kind::Browser,
    Kind::Tabs,
    Kind::Zoom,
    Kind::Media,
    Kind::Displays,
    Kind::Scanning,
];

/// Keys of menus that were retired. A rearrangement saved for one stays in
/// the file, so an older build still reads it, and is never used.
pub const RETIRED: [&str; 1] = ["more"];

pub fn editable(kind: Kind) -> bool {
    EDITABLE.contains(&kind)
}

/// The key a menu's override is stored under: the menu's serde name.
pub fn key(kind: Kind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .expect("kinds serialise to strings")
}

pub fn kind_from_key(key: &str) -> Option<Kind> {
    let kind: Kind = serde_json::from_value(serde_json::Value::String(key.into())).ok()?;
    editable(kind).then_some(kind)
}

/// The document id of a built-in menu, distinct from any user panel id.
pub fn id(kind: Kind) -> String {
    format!("builtin-{}", key(kind).to_lowercase())
}

/// A menu item as a tile. `None` for the point-scan items, which never
/// appear on an editable menu.
fn cell(item: Item) -> Option<Cell> {
    let action = match item {
        Item::Home => PanelAction::OpenPanel {
            panel: BuiltInPanel::Menu(Kind::Home),
        },
        Item::PointScan => PanelAction::OpenPanel {
            panel: BuiltInPanel::Point,
        },
        Item::Switchify => PanelAction::OpenPanel {
            panel: BuiltInPanel::Switchify,
        },
        Item::Keyboard => PanelAction::OpenPanel {
            panel: BuiltInPanel::Keyboard,
        },
        Item::MousePanel => PanelAction::OpenPanel {
            panel: BuiltInPanel::Mouse,
        },
        Item::Group(kind) => PanelAction::OpenPanel {
            panel: BuiltInPanel::Menu(kind),
        },
        Item::Command(command) => PanelAction::Command { command },
        Item::Setting(setting) => PanelAction::Setting { setting },
        Item::Display(next) => PanelAction::Display { next },
        Item::Pause => PanelAction::Pause,
        Item::Reverse => PanelAction::Reverse,
        Item::Cancel => PanelAction::Close,
        Item::Back => PanelAction::Back,
        Item::Panels => PanelAction::MyPanels,
        _ => return None,
    };
    Some(Cell {
        label: item.label().into(),
        action,
        span: 1,
        color: None,
    })
}

/// The menu as shipped. Home carries a My panels tile; the runtime leaves
/// it out while the user has no panels.
pub fn default_document(kind: Kind) -> CustomPanel {
    let mut rows: Vec<Vec<Cell>> = kind
        .rows()
        .into_iter()
        .map(|row| row.into_iter().filter_map(cell).collect())
        .filter(|row: &Vec<Cell>| !row.is_empty())
        .collect();
    if kind == Kind::Home {
        if let Some(last) = rows.last_mut() {
            last.insert(1, cell(Item::Panels).expect("My panels is a tile"));
        }
    }
    CustomPanel {
        id: id(kind),
        version: 1,
        name: kind.label().into(),
        description: String::new(),
        rows,
    }
}

/// Checks a rearranged menu. Its tiles follow the panel rules, it keeps a
/// way out, and Home keeps the Close tile that ends the scan.
pub fn validate_override(kind: Kind, panel: &CustomPanel) -> Result<(), String> {
    if !editable(kind) {
        return Err("That menu cannot be changed.".into());
    }
    if panel.id != id(kind) {
        return Err("Menu identity is invalid.".into());
    }
    panel.validate()?;
    let actions = || panel.rows.iter().flatten().map(|cell| &cell.action);
    if !actions().any(|action| matches!(action, PanelAction::Back | PanelAction::Close)) {
        return Err("Keep a Back or Close tile so the menu can be left.".into());
    }
    if kind == Kind::Home && !actions().any(|action| matches!(action, PanelAction::Close)) {
        return Err("Home keeps a Close tile, which ends scanning.".into());
    }
    Ok(())
}

/// Checks a whole set of overrides, as saved or synced.
pub fn validate_overrides(overrides: &BTreeMap<String, CustomPanel>) -> Result<(), String> {
    for (key, panel) in overrides {
        if RETIRED.contains(&key.as_str()) {
            panel.validate()?;
            continue;
        }
        let kind = kind_from_key(key).ok_or("That menu cannot be changed.")?;
        validate_override(kind, panel)?;
    }
    Ok(())
}

/// The document to scan for a menu: the override if there is one.
pub fn document(kind: Kind, overrides: &BTreeMap<Kind, CustomPanel>) -> CustomPanel {
    overrides
        .get(&kind)
        .map(|panel| upgraded(kind, panel))
        .unwrap_or_else(|| default_document(kind))
}

/// Whether a tile on the panel opens this menu.
fn opens(panel: &CustomPanel, kind: Kind) -> bool {
    panel.rows.iter().flatten().any(|cell| {
        cell.action
            == PanelAction::OpenPanel {
                panel: BuiltInPanel::Menu(kind),
            }
    })
}

/// A Home rearranged before it became the only hub has no Scanning tile,
/// and Displays and Scanning were reached through More actions. Such a
/// Home gains a row with them just above its last row. Home saved since
/// then keeps Scanning (see `validate_edit`), so it is left as it is.
pub fn upgraded(kind: Kind, panel: &CustomPanel) -> CustomPanel {
    let mut panel = panel.clone();
    if kind != Kind::Home || opens(&panel, Kind::Scanning) {
        return panel;
    }
    let mut row = Vec::new();
    if !opens(&panel, Kind::Displays) {
        row.extend(cell(Item::Group(Kind::Displays)));
    }
    row.extend(cell(Item::Group(Kind::Scanning)));
    if panel.rows.len() < crate::custom_panels::MAX_ROWS {
        let at = panel.rows.len().saturating_sub(1);
        panel.rows.insert(at, row);
    } else if let Some(last) = panel.rows.last_mut() {
        // A Home with every row in use takes what fits in its last row,
        // Scanning first, since it holds Pause and Open Switchify.
        let room = crate::custom_panels::MAX_CELLS_PER_ROW.saturating_sub(last.len());
        let keep = row.len().min(room);
        last.splice(0..0, row.split_off(row.len() - keep));
    }
    panel
}

/// Checks a menu the user saves in the editor. Beyond `validate_override`,
/// Home keeps Scanning, which holds Pause and Open Switchify.
pub fn validate_edit(kind: Kind, panel: &CustomPanel) -> Result<(), String> {
    validate_override(kind, panel)?;
    if kind == Kind::Home && !opens(panel, Kind::Scanning) {
        return Err("Home keeps Scanning, which holds Pause and Open Switchify.".into());
    }
    Ok(())
}

/// Overrides keyed by menu rather than by name, for the runtime.
pub fn by_kind(overrides: &BTreeMap<String, CustomPanel>) -> BTreeMap<Kind, CustomPanel> {
    overrides
        .iter()
        .filter_map(|(key, panel)| Some((kind_from_key(key)?, panel.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_editable_menu_has_a_valid_default_with_a_way_out() {
        for kind in EDITABLE {
            let document = default_document(kind);
            assert_eq!(validate_override(kind, &document), Ok(()), "{kind:?}");
            assert_eq!(kind_from_key(&key(kind)), Some(kind));
            assert!(crate::panel_actions::validate_panel_id(&document.id).is_ok());
            // Nothing was dropped: every item of the fixed rows is a tile.
            let items: usize = kind.rows().iter().map(Vec::len).sum();
            let tiles: usize = document.rows.iter().map(Vec::len).sum();
            assert_eq!(tiles, items + usize::from(kind == Kind::Home), "{kind:?}");
        }
        assert_eq!(kind_from_key("shutDown"), None);
        assert_eq!(kind_from_key("nonsense"), None);
    }

    #[test]
    fn home_ships_with_my_panels_beside_scanning_and_keeps_close() {
        let home = default_document(Kind::Home);
        let last: Vec<_> = home
            .rows
            .last()
            .unwrap()
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(last, ["Scanning", "My panels", "Close menu"]);
        let mut rearranged = home.clone();
        rearranged
            .rows
            .last_mut()
            .unwrap()
            .retain(|cell| cell.action != PanelAction::Close);
        assert!(validate_override(Kind::Home, &rearranged).is_err());
        let mut no_exit = default_document(Kind::Editing);
        no_exit.rows.last_mut().unwrap().clear();
        no_exit.rows.retain(|row| !row.is_empty());
        assert!(validate_override(Kind::Editing, &no_exit).is_err());
        let mut wrong_id = default_document(Kind::Editing);
        wrong_id.id = "editing".into();
        assert!(validate_override(Kind::Editing, &wrong_id).is_err());
        assert!(validate_override(Kind::ShutDown, &default_document(Kind::ShutDown)).is_err());
    }

    #[test]
    fn a_home_rearranged_before_it_held_scanning_gains_displays_and_scanning() {
        // As Home was shipped before: no Displays or Scanning, Close last.
        let mut old = default_document(Kind::Home);
        for row in &mut old.rows {
            row.retain(|cell| {
                !matches!(
                    cell.action,
                    PanelAction::OpenPanel {
                        panel: BuiltInPanel::Menu(Kind::Displays | Kind::Scanning)
                    }
                )
            });
        }
        old.rows.reverse();
        old.rows.retain(|row| !row.is_empty());
        let close_row = old.rows.len() - 1;
        old.rows.swap(0, close_row);
        assert!(validate_override(Kind::Home, &old).is_ok());
        assert!(validate_edit(Kind::Home, &old).is_err());
        let new = upgraded(Kind::Home, &old);
        assert_eq!(new.rows.len(), old.rows.len() + 1);
        let added: Vec<_> = new.rows[old.rows.len() - 1]
            .iter()
            .map(|cell| cell.label.as_str())
            .collect();
        assert_eq!(added, ["Displays", "Scanning"]);
        assert_eq!(new.rows.last(), old.rows.last());
        assert_eq!(validate_edit(Kind::Home, &new), Ok(()));
        // The runtime scans the upgraded Home.
        let by_kind = BTreeMap::from([(Kind::Home, old.clone())]);
        assert_eq!(document(Kind::Home, &by_kind), new);
        // A Home that already has Scanning, and every other menu, stay as saved.
        assert_eq!(upgraded(Kind::Home, &new), new);
        let editing = default_document(Kind::Editing);
        assert_eq!(upgraded(Kind::Editing, &editing), editing);
        // A Home with every row in use takes Scanning in its last row.
        let mut full = old.clone();
        let filler = full.rows[0].clone();
        while full.rows.len() < crate::custom_panels::MAX_ROWS {
            full.rows.insert(0, filler.clone());
        }
        let last = full.rows.last_mut().unwrap();
        while last.len() < crate::custom_panels::MAX_CELLS_PER_ROW - 1 {
            last.push(last[0].clone());
        }
        let new = upgraded(Kind::Home, &full);
        assert_eq!(new.rows.len(), crate::custom_panels::MAX_ROWS);
        assert_eq!(new.rows.last().unwrap()[0].label, "Scanning");
        assert_eq!(validate_edit(Kind::Home, &new), Ok(()));
    }

    #[test]
    fn overrides_are_keyed_by_menu_name_and_resolved_by_kind() {
        let mut overrides = BTreeMap::new();
        let mut home = default_document(Kind::Home);
        home.rows.reverse();
        overrides.insert(key(Kind::Home), home.clone());
        assert_eq!(validate_overrides(&overrides), Ok(()));
        let by_kind = by_kind(&overrides);
        assert_eq!(document(Kind::Home, &by_kind), home);
        assert_eq!(
            document(Kind::Editing, &by_kind),
            default_document(Kind::Editing)
        );
        // A rearrangement of the retired More actions page is kept, unused.
        let mut more = default_document(Kind::Home);
        more.id = "builtin-more".into();
        more.name = "More actions".into();
        overrides.insert("more".into(), more);
        assert_eq!(validate_overrides(&overrides), Ok(()));
        assert!(!super::by_kind(&overrides).contains_key(&Kind::More));
        overrides.insert("shutDown".into(), default_document(Kind::ShutDown));
        assert!(validate_overrides(&overrides).is_err());
    }
}
