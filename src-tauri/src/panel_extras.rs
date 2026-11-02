//! Extra rows of tiles below the built-in keys of the Mouse panel and the
//! Keyboard. The built-in keys never change; the user's rows sit under them,
//! above the panel's bottom row, and are stored and synced per panel.
use crate::{
    custom_panels::{Cell, MAX_CELLS_PER_ROW},
    panel_actions::PanelAction,
    scanning::TileRole,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Rows a panel takes below its keys. Few, so the panel stays on screen.
pub const MAX_ROWS: usize = 4;

/// A native panel that takes extra rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Mouse,
    Keyboard,
}

pub const ALL: [Target; 2] = [Target::Mouse, Target::Keyboard];

impl Target {
    /// The key the rows are stored under.
    pub fn key(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Keyboard => "keyboard",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse => "Mouse panel",
            Self::Keyboard => "Keyboard",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        ALL.into_iter().find(|target| target.key() == key)
    }
}

pub type Rows = Vec<Vec<Cell>>;
/// Extra rows by panel key. A panel without rows has no entry.
pub type Extras = BTreeMap<String, Rows>;

/// One of a panel's built-in keys, as the Panels page draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureCell {
    pub label: String,
    /// Width relative to a normal key of that panel.
    pub weight: f64,
}

/// The built-in keys of a panel, for a picture of it with its extra rows.
pub fn fixed_picture(target: Target) -> Vec<Vec<PictureCell>> {
    let rows = match target {
        Target::Mouse => crate::scan_mouse::MousePanel::fixed_picture(),
        Target::Keyboard => {
            crate::scan_keyboard::Keyboard::fixed_picture(cfg!(target_os = "macos"))
        }
    };
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|(label, weight)| PictureCell { label, weight })
                .collect()
        })
        .collect()
}

/// Trims the labels so equal rows compare equal.
pub fn normalized(mut rows: Rows) -> Rows {
    for cell in rows.iter_mut().flatten() {
        cell.label = cell.label.trim().into();
    }
    rows
}

/// Checks one panel's rows. No rows is fine: the panel is as shipped.
pub fn validate_rows(rows: &[Vec<Cell>]) -> Result<(), String> {
    if rows.len() > MAX_ROWS {
        return Err(format!("A panel takes up to {MAX_ROWS} extra rows."));
    }
    for (row_index, row) in rows.iter().enumerate() {
        if row.is_empty() || row.len() > MAX_CELLS_PER_ROW {
            return Err(format!(
                "Row {} needs 1 to {MAX_CELLS_PER_ROW} tiles.",
                row_index + 1
            ));
        }
        for (cell_index, cell) in row.iter().enumerate() {
            cell.validate(&format!("Row {}, tile {}", row_index + 1, cell_index + 1))?;
        }
    }
    Ok(())
}

/// Checks a whole set, as saved or synced. Every key names a panel that
/// takes extra rows, and no panel is listed with none.
pub fn validate_all(extras: &Extras) -> Result<(), String> {
    for (key, rows) in extras {
        Target::from_key(key).ok_or("That panel cannot be changed.")?;
        if rows.is_empty() {
            return Err("A panel without extra rows is not listed.".into());
        }
        validate_rows(rows)?;
    }
    Ok(())
}

/// Rows keyed by panel rather than by name, for the runtime.
pub fn by_target(extras: &Extras) -> BTreeMap<Target, Rows> {
    extras
        .iter()
        .filter_map(|(key, rows)| Some((Target::from_key(key)?, rows.clone())))
        .collect()
}

/// How a tile is drawn on a native panel: the ways out look like the
/// panel's own bottom row.
pub fn role(action: &PanelAction) -> TileRole {
    match action {
        PanelAction::Back | PanelAction::Close => TileRole::Toolbar,
        _ => TileRole::Utility,
    }
}

/// Whether choosing the tile leaves the panel it is on.
pub fn leaves(action: &PanelAction) -> bool {
    matches!(
        action,
        PanelAction::OpenPanel { .. }
            | PanelAction::OpenCustomPanel { .. }
            | PanelAction::MyPanels
            | PanelAction::Back
            | PanelAction::Close
    )
}

/// Prepares synced rows to replace `local`: validates them and drops
/// panels listed with no rows. Returns `None` when nothing would change.
pub fn plan_replacement(local: &Extras, incoming: Extras) -> Result<Option<Extras>, String> {
    let incoming: Extras = incoming
        .into_iter()
        .map(|(key, rows)| (key, normalized(rows)))
        .filter(|(_, rows)| !rows.is_empty())
        .collect();
    validate_all(&incoming)?;
    if incoming == *local {
        return Ok(None);
    }
    Ok(Some(incoming))
}

/// Three-way merge, one panel at a time: a panel changed on one side wins,
/// changed on both the account wins, and rows added on one side are never
/// lost to a removal on the other.
pub fn merge(base_local: &Extras, base_cloud: &Extras, local: &Extras, cloud: &Extras) -> Extras {
    let keys: std::collections::BTreeSet<&String> = local.keys().chain(cloud.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let (l, c) = (local.get(key), cloud.get(key));
            let local_changed = l != base_local.get(key);
            let cloud_changed = c != base_cloud.get(key);
            let chosen = match (local_changed, cloud_changed) {
                (false, _) => c,
                (true, false) => l,
                (true, true) => c.or(l),
            }?;
            Some((key.clone(), chosen.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn tile(label: &str, action: PanelAction) -> Cell {
        Cell {
            label: label.into(),
            action,
            span: 1,
        }
    }

    #[test]
    fn rows_are_checked_like_panel_rows_and_capped() {
        let hello = tile(
            "Hello",
            PanelAction::Text {
                text: "Hello".into(),
            },
        );
        assert_eq!(validate_rows(&[]), Ok(()));
        assert_eq!(validate_rows(&[vec![hello.clone()]]), Ok(()));
        assert!(validate_rows(&vec![vec![hello.clone()]; MAX_ROWS + 1]).is_err());
        assert!(validate_rows(&[vec![]]).is_err());
        let mut blank = hello.clone();
        blank.label = " ".into();
        assert!(validate_rows(&[vec![blank]]).is_err());
        let mut wide = hello.clone();
        wide.span = 5;
        assert!(validate_rows(&[vec![wide]]).is_err());

        let mut extras = Extras::new();
        extras.insert("mouse".into(), vec![vec![hello.clone()]]);
        assert_eq!(validate_all(&extras), Ok(()));
        assert_eq!(
            by_target(&extras).keys().copied().collect::<Vec<_>>(),
            [Target::Mouse]
        );
        extras.insert("keyboard".into(), vec![]);
        assert!(validate_all(&extras).is_err());
        extras.remove("keyboard");
        extras.insert("home".into(), vec![vec![hello]]);
        assert!(validate_all(&extras).is_err());
        assert_eq!(Target::from_key("keyboard"), Some(Target::Keyboard));
        assert_eq!(Target::from_key("Mouse"), None);
    }

    #[test]
    fn ways_out_are_drawn_as_toolbar_keys_and_leave_the_panel() {
        assert_eq!(role(&PanelAction::Close), TileRole::Toolbar);
        assert_eq!(role(&PanelAction::Pause), TileRole::Utility);
        assert!(leaves(&PanelAction::Back));
        assert!(leaves(&PanelAction::MyPanels));
        assert!(!leaves(&PanelAction::Text { text: "x".into() }));
        assert!(!leaves(&PanelAction::Reverse));
    }

    #[test]
    fn synced_rows_replace_only_when_different_and_merge_per_panel() {
        let hello = vec![vec![tile(
            " Hello ",
            PanelAction::Text {
                text: "Hello".into(),
            },
        )]];
        let trimmed = normalized(hello.clone());
        let mut local = Extras::new();
        local.insert("mouse".into(), trimmed.clone());
        let mut incoming = Extras::new();
        incoming.insert("mouse".into(), hello.clone());
        incoming.insert("keyboard".into(), vec![]);
        assert_eq!(plan_replacement(&local, incoming.clone()), Ok(None));
        incoming.insert("keyboard".into(), hello.clone());
        let planned = plan_replacement(&local, incoming).unwrap().unwrap();
        assert_eq!(planned["keyboard"], trimmed);
        let mut bad = Extras::new();
        bad.insert("editing".into(), hello.clone());
        assert!(plan_replacement(&local, bad).is_err());

        // Added here, removed there since the base: the addition stays.
        let base = Extras::new();
        let mut cloud = local.clone();
        cloud.remove("mouse");
        assert_eq!(merge(&base, &local, &local, &cloud), local);
        // Changed on both sides: the account wins.
        let mut changed = local.clone();
        changed.get_mut("mouse").unwrap()[0][0].label = "Hi".into();
        assert_eq!(merge(&base, &base, &local, &changed), changed);
        // Removed on one side, untouched on the other: removed.
        assert_eq!(merge(&local, &local, &local, &base), base);
    }

    #[test]
    fn pictures_show_the_built_in_keys() {
        let mouse = fixed_picture(Target::Mouse);
        assert_eq!(mouse.len(), 5);
        assert_eq!(mouse[0][0].label, "Left click");
        assert_eq!(mouse[4].last().unwrap().label, "Switch to Point");
        let keyboard = fixed_picture(Target::Keyboard);
        assert!(keyboard.len() >= 5);
        assert!(keyboard
            .iter()
            .flatten()
            .any(|cell| cell.label == "Space" && cell.weight > 1.0));
    }
}
