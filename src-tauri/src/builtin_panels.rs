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
pub const EDITABLE: [Kind; 11] = [
    Kind::Home,
    Kind::More,
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
        Item::More => PanelAction::OpenPanel {
            panel: BuiltInPanel::Menu(Kind::More),
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
        let kind = kind_from_key(key).ok_or("That menu cannot be changed.")?;
        validate_override(kind, panel)?;
    }
    Ok(())
}

/// The document to scan for a menu: the override if there is one.
pub fn document(kind: Kind, overrides: &BTreeMap<Kind, CustomPanel>) -> CustomPanel {
    overrides
        .get(&kind)
        .cloned()
        .unwrap_or_else(|| default_document(kind))
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
    fn home_ships_with_my_panels_beside_switchify_and_keeps_close() {
        let home = default_document(Kind::Home);
        let last: Vec<_> = home
            .rows
            .last()
            .unwrap()
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(last, ["Switchify", "My panels", "Close menu"]);
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
        overrides.insert("shutDown".into(), default_document(Kind::ShutDown));
        assert!(validate_overrides(&overrides).is_err());
    }
}
