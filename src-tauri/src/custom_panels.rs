//! User-built scanning panels: the saved shape, its limits, the local store
//! and the per-id merge used by settings sync. No native windows or input.
use crate::panel_actions::{self, PanelAction};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT: &str = "custom-panels-changed";
pub const MAX_PANELS: usize = 32;
pub const MAX_ROWS: usize = 12;
pub const MAX_CELLS_PER_ROW: usize = 8;
pub const MAX_NAME_CHARS: usize = 50;
pub const MAX_DESCRIPTION_CHARS: usize = 200;
pub const MAX_LABEL_CHARS: usize = 40;
const MIN_WEIGHT: f64 = 0.5;
const MAX_WEIGHT: f64 = 4.0;
const STORE_SCHEMA: u32 = 1;

/// One tile. The label is what people read; the action is what Select does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cell {
    pub label: String,
    pub action: PanelAction,
    /// Share of the row's width relative to its neighbours.
    pub weight: f64,
}

/// A saved panel. `version` rises on every change so other computers and
/// caches notice; `rows` are ragged, each row sharing its width by weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomPanel {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub rows: Vec<Vec<Cell>>,
}

impl CustomPanel {
    /// Everything except the version, which the store owns.
    pub fn same_content(&self, other: &Self) -> bool {
        self.name == other.name && self.description == other.description && self.rows == other.rows
    }

    /// Checks the panel alone. Links to panels that do not exist are allowed
    /// (a linked panel may be deleted, or arrive later from sync); the
    /// runtime reports them when selected.
    pub fn validate(&self) -> Result<(), String> {
        panel_actions::validate_panel_id(&self.id)?;
        if self.version == 0 {
            return Err("Panel version must be 1 or more.".into());
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Give the panel a name.".into());
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(format!(
                "Panel names use {MAX_NAME_CHARS} characters or fewer."
            ));
        }
        if self.description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(format!(
                "Panel descriptions use {MAX_DESCRIPTION_CHARS} characters or fewer."
            ));
        }
        if self.rows.is_empty() || self.rows.len() > MAX_ROWS {
            return Err(format!("A panel has 1 to {MAX_ROWS} rows."));
        }
        for (row_index, row) in self.rows.iter().enumerate() {
            if row.is_empty() || row.len() > MAX_CELLS_PER_ROW {
                return Err(format!(
                    "Row {} needs 1 to {MAX_CELLS_PER_ROW} tiles.",
                    row_index + 1
                ));
            }
            for (cell_index, cell) in row.iter().enumerate() {
                let place = format!("Row {}, tile {}", row_index + 1, cell_index + 1);
                let label = cell.label.trim();
                if label.is_empty() {
                    return Err(format!("{place}: give the tile a label."));
                }
                if label.chars().count() > MAX_LABEL_CHARS {
                    return Err(format!(
                        "{place}: labels use {MAX_LABEL_CHARS} characters or fewer."
                    ));
                }
                if label.chars().any(|c| c.is_control() && c != '\n') {
                    return Err(format!("{place}: the label has unusable characters."));
                }
                if !cell.weight.is_finite() || !(MIN_WEIGHT..=MAX_WEIGHT).contains(&cell.weight) {
                    return Err(format!(
                        "{place}: tile width is between {MIN_WEIGHT} and {MAX_WEIGHT}."
                    ));
                }
                cell.action
                    .validate()
                    .map_err(|error| format!("{place}: {error}"))?;
            }
        }
        Ok(())
    }

    /// Trims the fields people type so equal panels compare equal.
    pub fn normalized(mut self) -> Self {
        self.name = self.name.trim().into();
        self.description = self.description.trim().into();
        for cell in self.rows.iter_mut().flatten() {
            cell.label = cell.label.trim().into();
        }
        self
    }
}

/// Checks a whole set: each panel, the count, and unique ids and names.
pub fn validate_set(panels: &[CustomPanel]) -> Result<(), String> {
    if panels.len() > MAX_PANELS {
        return Err(format!("No more than {MAX_PANELS} panels can be saved."));
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for panel in panels {
        panel.validate()?;
        if !ids.insert(panel.id.as_str()) {
            return Err("Panel identity is invalid.".into());
        }
        if !names.insert(panel.name.trim().to_lowercase()) {
            return Err("Panel names must be unique.".into());
        }
    }
    Ok(())
}

/// The file on disk. Separate from app state so a panel edit never rewrites
/// pairing or settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Store {
    schema_version: u32,
    panels: Vec<CustomPanel>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            schema_version: STORE_SCHEMA,
            panels: Vec::new(),
        }
    }
}

pub struct Controller {
    panels: Mutex<Vec<CustomPanel>>,
}

fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("panels.json"))
}

/// Reads the saved panels, dropping the file's contents if any panel is
/// invalid rather than running with a half-usable set.
fn load(bytes: &[u8]) -> Option<Vec<CustomPanel>> {
    let store: Store = serde_json::from_slice(bytes).ok()?;
    if store.schema_version != STORE_SCHEMA {
        return None;
    }
    validate_set(&store.panels).ok()?;
    Some(store.panels)
}

pub fn install(app: &AppHandle) {
    let panels = path(app)
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| load(&bytes))
        .unwrap_or_default();
    app.manage(Controller {
        panels: Mutex::new(panels),
    });
}

pub fn list(app: &AppHandle) -> Vec<CustomPanel> {
    app.state::<Controller>()
        .panels
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

fn write(app: &AppHandle, panels: &[CustomPanel]) -> Result<(), String> {
    let p = path(app)?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let store = Store {
        schema_version: STORE_SCHEMA,
        panels: panels.to_vec(),
    };
    let temp = p.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    crate::storage::replace_file(&temp, &p)
}

/// Adds or replaces one panel. The version becomes one above the saved
/// copy's, whatever the caller sent, so a stale editor can never lower it.
pub fn save(app: &AppHandle, panel: CustomPanel) -> Result<Vec<CustomPanel>, String> {
    let mut panel = panel.normalized();
    panel.validate()?;
    let controller = app.state::<Controller>();
    let mut panels = controller.panels.lock().unwrap_or_else(|p| p.into_inner());
    let existing = panels.iter().position(|candidate| candidate.id == panel.id);
    if existing.is_none() && panels.len() >= MAX_PANELS {
        return Err(format!("No more than {MAX_PANELS} panels can be saved."));
    }
    panel.version = existing.map_or(1, |index| panels[index].version.saturating_add(1));
    let mut next = panels.clone();
    match existing {
        Some(index) => next[index] = panel,
        None => next.push(panel),
    }
    validate_set(&next)?;
    write(app, &next)?;
    *panels = next;
    drop(panels);
    let _ = app.emit(EVENT, ());
    Ok(list(app))
}

pub fn delete(app: &AppHandle, id: &str) -> Result<Vec<CustomPanel>, String> {
    let controller = app.state::<Controller>();
    let mut panels = controller.panels.lock().unwrap_or_else(|p| p.into_inner());
    if !panels.iter().any(|panel| panel.id == id) {
        return Err("That panel no longer exists.".into());
    }
    let next: Vec<_> = panels
        .iter()
        .filter(|panel| panel.id != id)
        .cloned()
        .collect();
    write(app, &next)?;
    *panels = next;
    drop(panels);
    let _ = app.emit(EVENT, ());
    Ok(list(app))
}

/// Replaces every panel with a synced set that `plan_replacement` prepared.
pub fn replace(app: &AppHandle, panels: Vec<CustomPanel>) -> Result<(), String> {
    validate_set(&panels)?;
    let controller = app.state::<Controller>();
    let mut current = controller.panels.lock().unwrap_or_else(|p| p.into_inner());
    write(app, &panels)?;
    *current = panels;
    drop(current);
    let _ = app.emit(EVENT, ());
    Ok(())
}

/// Prepares an incoming set to replace `local`: validates it, and gives a
/// changed panel a version above both copies while an unchanged one keeps
/// the higher. Returns `None` when nothing would change.
pub fn plan_replacement(
    local: &[CustomPanel],
    incoming: Vec<CustomPanel>,
) -> Result<Option<Vec<CustomPanel>>, String> {
    let mut incoming: Vec<_> = incoming.into_iter().map(CustomPanel::normalized).collect();
    validate_set(&incoming)?;
    for panel in &mut incoming {
        if let Some(existing) = local.iter().find(|candidate| candidate.id == panel.id) {
            panel.version = if existing.same_content(panel) {
                existing.version.max(panel.version)
            } else {
                panel.version.max(existing.version.saturating_add(1))
            };
        }
    }
    if local.len() == incoming.len() && local.iter().zip(&incoming).all(|(a, b)| a == b) {
        return Ok(None);
    }
    Ok(Some(incoming))
}

/// Three-way merge by id, the same rules as switch profiles: a panel changed
/// on one side wins, changed on both the account wins, and an edit is never
/// lost to a deletion on the other side. Names made equal by the merge get
/// a numbered suffix.
pub fn merge(
    base_local: &[CustomPanel],
    base_cloud: &[CustomPanel],
    local: &[CustomPanel],
    cloud: &[CustomPanel],
) -> Vec<CustomPanel> {
    let find = |list: &[CustomPanel], id: &str| list.iter().find(|p| p.id == id).cloned();
    let mut ids: Vec<String> = cloud.iter().map(|p| p.id.clone()).collect();
    ids.extend(
        local
            .iter()
            .filter(|p| !cloud.iter().any(|c| c.id == p.id))
            .map(|p| p.id.clone()),
    );
    let changed = |a: &Option<CustomPanel>, b: &Option<CustomPanel>| match (a, b) {
        (Some(a), Some(b)) => !a.same_content(b),
        (None, None) => false,
        _ => true,
    };
    let mut merged: Vec<CustomPanel> = ids
        .iter()
        .filter_map(|id| {
            let (bl, bc) = (find(base_local, id), find(base_cloud, id));
            let (l, c) = (find(local, id), find(cloud, id));
            let local_changed = changed(&l, &bl);
            let cloud_changed = changed(&c, &bc);
            let mut chosen = match (local_changed, cloud_changed) {
                (false, _) => c.clone(),
                (true, false) => l.clone(),
                (true, true) => c.clone().or(l.clone()),
            }?;
            chosen.version = [&l, &c]
                .into_iter()
                .flatten()
                .map(|p| p.version)
                .max()
                .unwrap_or(chosen.version);
            Some(chosen)
        })
        .collect();
    let mut seen = HashSet::new();
    for panel in &mut merged {
        if !seen.insert(panel.name.to_lowercase()) {
            let base: String = panel.name.chars().take(MAX_NAME_CHARS - 6).collect();
            let name = (2..)
                .map(|n| format!("{base} ({n})"))
                .find(|candidate| !seen.contains(&candidate.to_lowercase()))
                .expect("an unused suffix always exists");
            seen.insert(name.to_lowercase());
            panel.name = name;
        }
    }
    merged
}

#[cfg(test)]
pub(crate) fn sample(id: &str, name: &str) -> CustomPanel {
    CustomPanel {
        id: id.into(),
        version: 1,
        name: name.into(),
        description: String::new(),
        rows: vec![
            vec![
                Cell {
                    label: "Copy".into(),
                    action: PanelAction::Command {
                        command: crate::scan_menu::Command::Copy,
                    },
                    weight: 1.0,
                },
                Cell {
                    label: "Hello".into(),
                    action: PanelAction::Text {
                        text: "Hello there".into(),
                    },
                    weight: 2.0,
                },
            ],
            vec![Cell {
                label: "Back".into(),
                action: PanelAction::Back,
                weight: 1.0,
            }],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_round_trips_with_a_stable_shape() {
        let panel = sample("greetings", "Greetings");
        let value = serde_json::to_value(&panel).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "id": "greetings",
                "version": 1,
                "name": "Greetings",
                "description": "",
                "rows": [
                    [
                        {"label": "Copy", "action": {"type": "command", "command": "copy"}, "weight": 1.0},
                        {"label": "Hello", "action": {"type": "text", "text": "Hello there"}, "weight": 2.0}
                    ],
                    [{"label": "Back", "action": {"type": "back"}, "weight": 1.0}]
                ]
            })
        );
        assert_eq!(serde_json::from_value::<CustomPanel>(value).unwrap(), panel);
        let mut extra = serde_json::to_value(&panel).unwrap();
        extra["colour"] = serde_json::json!("red");
        assert!(serde_json::from_value::<CustomPanel>(extra).is_err());
    }

    #[test]
    fn validation_covers_ids_names_rows_tiles_and_actions() {
        assert_eq!(sample("ok", "Ok").validate(), Ok(()));
        let mut panel = sample("Bad Id", "Ok");
        assert!(panel.validate().is_err());
        panel = sample("ok", "   ");
        assert!(panel.validate().is_err());
        panel = sample("ok", &"n".repeat(MAX_NAME_CHARS + 1));
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.version = 0;
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.description = "d".repeat(MAX_DESCRIPTION_CHARS + 1);
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows.clear();
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows = vec![vec![]; 1];
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows = vec![panel.rows[1].clone(); MAX_ROWS + 1];
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows = vec![vec![panel.rows[1][0].clone(); MAX_CELLS_PER_ROW + 1]];
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows[0][0].label = String::new();
        let error = panel.validate().unwrap_err();
        assert!(error.starts_with("Row 1, tile 1"), "{error}");
        panel = sample("ok", "Ok");
        panel.rows[0][0].weight = 0.1;
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows[0][0].weight = f64::NAN;
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows[0][1].action = PanelAction::Text {
            text: "secret\u{7}".into(),
        };
        let error = panel.validate().unwrap_err();
        assert!(error.starts_with("Row 1, tile 2"), "{error}");
        assert!(!error.contains("secret"));
        // Links to a missing panel are allowed.
        panel = sample("ok", "Ok");
        panel.rows[0][0].action = PanelAction::OpenCustomPanel {
            id: "elsewhere".into(),
        };
        assert_eq!(panel.validate(), Ok(()));
    }

    #[test]
    fn a_set_needs_unique_ids_and_names_within_the_limit() {
        assert_eq!(validate_set(&[]), Ok(()));
        assert!(validate_set(&[sample("a", "One"), sample("a", "Two")]).is_err());
        assert!(validate_set(&[sample("a", "One"), sample("b", " one ")]).is_err());
        assert_eq!(
            validate_set(&[sample("a", "One"), sample("b", "Two")]),
            Ok(())
        );
        let many: Vec<_> = (0..=MAX_PANELS)
            .map(|n| sample(&format!("p{n}"), &format!("Panel {n}")))
            .collect();
        assert!(validate_set(&many).is_err());
    }

    #[test]
    fn the_store_rejects_other_schemas_and_invalid_panels() {
        let good = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA,
            panels: vec![sample("a", "A")],
        })
        .unwrap();
        assert_eq!(load(&good).unwrap().len(), 1);
        let newer = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA + 1,
            panels: vec![],
        })
        .unwrap();
        assert!(load(&newer).is_none());
        let mut bad = sample("a", "A");
        bad.rows.clear();
        let invalid = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA,
            panels: vec![bad],
        })
        .unwrap();
        assert!(load(&invalid).is_none());
        assert!(load(b"not json").is_none());
    }

    #[test]
    fn replacement_keeps_versions_moving_forward() {
        let local = vec![sample("a", "A")];
        // Same content: nothing to do.
        assert_eq!(plan_replacement(&local, local.clone()), Ok(None));
        // Same content, higher incoming version: adopt the higher.
        let mut incoming = sample("a", "A");
        incoming.version = 5;
        let planned = plan_replacement(&local, vec![incoming]).unwrap().unwrap();
        assert_eq!(planned[0].version, 5);
        // Changed content with a low version: one above the local copy.
        let mut local_v4 = sample("a", "A");
        local_v4.version = 4;
        let mut changed = sample("a", "A renamed");
        changed.version = 1;
        let planned = plan_replacement(&[local_v4], vec![changed])
            .unwrap()
            .unwrap();
        assert_eq!(planned[0].version, 5);
        // An invalid incoming set is refused before anything changes.
        let mut bad = sample("b", "B");
        bad.rows.clear();
        assert!(plan_replacement(&local, vec![bad]).is_err());
    }

    #[test]
    fn merge_keeps_additions_from_both_sides_and_never_loses_an_edit() {
        let a = sample("a", "Desk");
        let b = sample("b", "Laptop");
        let c = sample("c", "Tablet");
        let merged = merge(
            std::slice::from_ref(&a),
            std::slice::from_ref(&a),
            &[a.clone(), b.clone()],
            &[a.clone(), c.clone()],
        );
        let names: Vec<_> = merged.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Desk", "Tablet", "Laptop"]);

        // Deleted on the account, unchanged here: stays deleted.
        let merged = merge(
            &[a.clone(), b.clone()],
            &[a.clone(), b.clone()],
            &[a.clone(), b.clone()],
            std::slice::from_ref(&a),
        );
        assert_eq!(merged.len(), 1);

        // Deleted on the account but edited here: the edit survives.
        let mut edited = b.clone();
        edited.description = "Edited".into();
        let merged = merge(
            &[a.clone(), b.clone()],
            &[a.clone(), b.clone()],
            &[a.clone(), edited.clone()],
            std::slice::from_ref(&a),
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].description, "Edited");

        // Edited on both: the account wins, the version never goes backwards.
        let mut local_edit = a.clone();
        local_edit.name = "Desk at home".into();
        local_edit.version = 3;
        let mut cloud_edit = a.clone();
        cloud_edit.name = "Desk at work".into();
        cloud_edit.version = 2;
        let merged = merge(
            std::slice::from_ref(&a),
            std::slice::from_ref(&a),
            &[local_edit],
            &[cloud_edit],
        );
        assert_eq!(merged[0].name, "Desk at work");
        assert_eq!(merged[0].version, 3);

        // Names made equal by the merge get a suffix.
        let mut renamed = b.clone();
        renamed.name = "Desk".into();
        let merged = merge(
            std::slice::from_ref(&a),
            std::slice::from_ref(&a),
            &[a.clone(), renamed],
            std::slice::from_ref(&a),
        );
        assert_eq!(merged[1].name, "Desk (2)");
    }
}
