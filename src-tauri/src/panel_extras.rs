//! Extra tiles around the built-in keys of the Mouse panel and the Keyboard:
//! rows below the keys and columns to their right. The built-in keys never
//! change; the user's tiles are stored and synced per panel.
use crate::{
    custom_panels::{Cell, MAX_CELLS_PER_ROW},
    panel_actions::PanelAction,
    scanning::TileRole,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Rows a panel takes below its keys. Few, so the panel stays on screen.
pub const MAX_ROWS: usize = 4;
/// Columns a panel takes beside its keys, and tiles in each.
pub const MAX_COLUMNS: usize = 2;
pub const MAX_COLUMN_CELLS: usize = 6;

/// A native panel that takes extra tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Mouse,
    Keyboard,
}

pub const ALL: [Target; 2] = [Target::Mouse, Target::Keyboard];

impl Target {
    /// The key the tiles are stored under.
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

/// The tiles a user added around one panel's keys. Rows run below the keys;
/// each column runs down the right of the panel from its top row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelExtras {
    #[serde(default)]
    pub rows: Rows,
    #[serde(default)]
    pub columns: Rows,
}

impl PanelExtras {
    #[cfg(test)]
    pub fn rows(rows: Rows) -> Self {
        Self {
            rows,
            columns: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.columns.is_empty()
    }
    /// Trims the labels so equal tiles compare equal.
    pub fn normalized(mut self) -> Self {
        for cell in self
            .rows
            .iter_mut()
            .chain(self.columns.iter_mut())
            .flatten()
        {
            cell.label = cell.label.trim().into();
        }
        self
    }
    /// Checks one panel's tiles. None at all is fine: the panel is as it came.
    pub fn validate(&self) -> Result<(), String> {
        validate_rows(&self.rows)?;
        validate_columns(&self.columns)
    }
}

/// Extra tiles by panel key. A panel without any has no entry.
pub type Extras = BTreeMap<String, PanelExtras>;

/// One of a panel's built-in keys, as the Panels page draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureCell {
    pub label: String,
    /// Width relative to a normal key of that panel.
    pub weight: f64,
}

/// The built-in keys of a panel, for a picture of it with its extra tiles.
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

/// Checks the rows below a panel's keys.
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

/// Checks the columns beside a panel's keys.
pub fn validate_columns(columns: &[Vec<Cell>]) -> Result<(), String> {
    if columns.len() > MAX_COLUMNS {
        return Err(format!("A panel takes up to {MAX_COLUMNS} extra columns."));
    }
    for (column_index, column) in columns.iter().enumerate() {
        if column.is_empty() || column.len() > MAX_COLUMN_CELLS {
            return Err(format!(
                "Column {} needs 1 to {MAX_COLUMN_CELLS} tiles.",
                column_index + 1
            ));
        }
        for (cell_index, cell) in column.iter().enumerate() {
            cell.validate(&format!(
                "Column {}, tile {}",
                column_index + 1,
                cell_index + 1
            ))?;
        }
    }
    Ok(())
}

/// Checks a whole set, as saved or synced. Every key names a panel that
/// takes extra tiles, and no panel is listed with none.
pub fn validate_all(extras: &Extras) -> Result<(), String> {
    for (key, panel) in extras {
        Target::from_key(key).ok_or("That panel cannot be changed.")?;
        if panel.is_empty() {
            return Err("A panel without extra tiles is not listed.".into());
        }
        panel.validate()?;
    }
    Ok(())
}

/// Tiles keyed by panel rather than by name, for the runtime.
pub fn by_target(extras: &Extras) -> BTreeMap<Target, PanelExtras> {
    extras
        .iter()
        .filter_map(|(key, panel)| Some((Target::from_key(key)?, panel.clone())))
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

/// Prepares synced tiles to replace `local`: validates them and drops
/// panels listed with none. Returns `None` when nothing would change.
pub fn plan_replacement(local: &Extras, incoming: Extras) -> Result<Option<Extras>, String> {
    let incoming: Extras = incoming
        .into_iter()
        .map(|(key, panel)| (key, panel.normalized()))
        .filter(|(_, panel)| !panel.is_empty())
        .collect();
    validate_all(&incoming)?;
    if incoming == *local {
        return Ok(None);
    }
    Ok(Some(incoming))
}

/// Three-way merge, one panel at a time: a panel changed on one side wins,
/// changed on both the account wins, and tiles added on one side are never
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
    fn rows_and_columns_are_checked_like_panel_rows_and_capped() {
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
        assert_eq!(validate_columns(&[vec![hello.clone()]]), Ok(()));
        assert!(validate_columns(&vec![vec![hello.clone()]; MAX_COLUMNS + 1]).is_err());
        assert!(validate_columns(&[vec![hello.clone(); MAX_COLUMN_CELLS + 1]]).is_err());
        assert!(validate_columns(&[vec![]]).is_err());

        let mut extras = Extras::new();
        extras.insert("mouse".into(), PanelExtras::rows(vec![vec![hello.clone()]]));
        assert_eq!(validate_all(&extras), Ok(()));
        assert_eq!(
            by_target(&extras).keys().copied().collect::<Vec<_>>(),
            [Target::Mouse]
        );
        extras.insert("keyboard".into(), PanelExtras::default());
        assert!(validate_all(&extras).is_err());
        extras.remove("keyboard");
        extras.insert("home".into(), PanelExtras::rows(vec![vec![hello]]));
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
    fn synced_tiles_replace_only_when_different_and_merge_per_panel() {
        let hello = vec![vec![tile(
            " Hello ",
            PanelAction::Text {
                text: "Hello".into(),
            },
        )]];
        let trimmed = PanelExtras::rows(hello.clone()).normalized();
        let mut local = Extras::new();
        local.insert("mouse".into(), trimmed.clone());
        let mut incoming = Extras::new();
        incoming.insert("mouse".into(), PanelExtras::rows(hello.clone()));
        incoming.insert("keyboard".into(), PanelExtras::default());
        assert_eq!(plan_replacement(&local, incoming.clone()), Ok(None));
        incoming.insert(
            "keyboard".into(),
            PanelExtras {
                rows: Vec::new(),
                columns: hello.clone(),
            },
        );
        let planned = plan_replacement(&local, incoming).unwrap().unwrap();
        assert_eq!(planned["keyboard"].columns, trimmed.rows);
        let mut bad = Extras::new();
        bad.insert("editing".into(), PanelExtras::rows(hello.clone()));
        assert!(plan_replacement(&local, bad).is_err());

        // Added here, removed there since the base: the addition stays.
        let base = Extras::new();
        let mut cloud = local.clone();
        cloud.remove("mouse");
        assert_eq!(merge(&base, &local, &local, &cloud), local);
        // Changed on both sides: the account wins.
        let mut changed = local.clone();
        changed.get_mut("mouse").unwrap().rows[0][0].label = "Hi".into();
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
