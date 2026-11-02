//! User-built scanning panels: the saved shape, its limits, the local store
//! and the per-id merge used by settings sync. No native windows or input.
use crate::panel_actions::{self, PanelAction};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT: &str = "custom-panels-changed";
pub const MAX_PANELS: usize = 32;
pub const MAX_ROWS: usize = 12;
pub const MAX_CELLS_PER_ROW: usize = 8;
pub const MAX_NAME_CHARS: usize = 50;
pub const MAX_DESCRIPTION_CHARS: usize = 200;
pub const MAX_LABEL_CHARS: usize = 40;
/// Widest a tile may be, in units of a normal tile.
pub const MAX_SPAN: u8 = 4;
/// Version 2 added the built-in menu overrides and the extra rows on the
/// mouse panel and the keyboard; a version 1 file still loads.
const STORE_SCHEMA: u32 = 2;

/// One tile. The label is what people read; the action is what Select does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cell {
    pub label: String,
    pub action: PanelAction,
    /// Width in units of a normal tile, 1 to 4. The row shares its width
    /// out by span. An integer, so the synced document round-trips exactly.
    pub span: u8,
}

/// A saved panel. `version` rises on every change so other computers and
/// caches notice; `rows` are ragged, each row sharing its width by span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomPanel {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub rows: Vec<Vec<Cell>>,
}

impl Cell {
    /// Checks one tile. `place` names it in any error.
    pub fn validate(&self, place: &str) -> Result<(), String> {
        let label = self.label.trim();
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
        if !(1..=MAX_SPAN).contains(&self.span) {
            return Err(format!("{place}: tile width is 1 to {MAX_SPAN}."));
        }
        self.action
            .validate()
            .map_err(|error| format!("{place}: {error}"))
    }
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
        if name.chars().any(char::is_control) {
            return Err("The panel name has unusable characters.".into());
        }
        if self.description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(format!(
                "Panel descriptions use {MAX_DESCRIPTION_CHARS} characters or fewer."
            ));
        }
        if self
            .description
            .chars()
            .any(|c| c.is_control() && c != '\n')
        {
            return Err("The panel description has unusable characters.".into());
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
                cell.validate(&format!("Row {}, tile {}", row_index + 1, cell_index + 1))?;
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
    /// Rearranged built-in menus, keyed by menu name (`builtin_panels::key`).
    #[serde(default)]
    overrides: BTreeMap<String, CustomPanel>,
    /// Extra rows on the mouse panel and the keyboard, by panel
    /// (`panel_extras::Target::key`).
    #[serde(default)]
    extras: crate::panel_extras::Extras,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            schema_version: STORE_SCHEMA,
            panels: Vec::new(),
            overrides: BTreeMap::new(),
            extras: BTreeMap::new(),
        }
    }
}

/// What the store holds: the user's panels, the rearranged menus and the
/// extra rows on the native panels.
#[derive(Debug, Clone, Default, PartialEq)]
struct Contents {
    panels: Vec<CustomPanel>,
    overrides: BTreeMap<String, CustomPanel>,
    extras: crate::panel_extras::Extras,
}

pub struct Controller {
    contents: Mutex<Contents>,
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
fn load(bytes: &[u8]) -> Option<Contents> {
    let store: Store = serde_json::from_slice(bytes).ok()?;
    if !(1..=STORE_SCHEMA).contains(&store.schema_version) {
        return None;
    }
    validate_set(&store.panels).ok()?;
    crate::builtin_panels::validate_overrides(&store.overrides).ok()?;
    crate::panel_extras::validate_all(&store.extras).ok()?;
    Some(Contents {
        panels: store.panels,
        overrides: store.overrides,
        extras: store.extras,
    })
}

/// Loads the saved panels. A file this build cannot use (damaged, or
/// written by a newer build) is set aside as `panels.json.bad` rather than
/// overwritten by the next save, so nothing is lost for good.
pub fn install(app: &AppHandle) {
    let contents = path(app).ok().and_then(|p| {
        let bytes = std::fs::read(&p).ok()?;
        let loaded = load(&bytes);
        if loaded.is_none() {
            let _ = std::fs::rename(&p, p.with_extension("json.bad"));
        }
        loaded
    });
    app.manage(Controller {
        contents: Mutex::new(contents.unwrap_or_default()),
    });
}

fn contents(app: &AppHandle) -> Contents {
    app.state::<Controller>()
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

pub fn list(app: &AppHandle) -> Vec<CustomPanel> {
    contents(app).panels
}

/// The rearranged built-in menus, keyed by menu name.
pub fn overrides(app: &AppHandle) -> BTreeMap<String, CustomPanel> {
    contents(app).overrides
}

/// The extra rows on the mouse panel and the keyboard, keyed by panel.
pub fn extras(app: &AppHandle) -> crate::panel_extras::Extras {
    contents(app).extras
}

fn write(app: &AppHandle, contents: &Contents) -> Result<(), String> {
    let p = path(app)?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    // A file with only panels stays version 1, so an older build can still
    // read it after a rollback.
    let store = Store {
        schema_version: if contents.overrides.is_empty() && contents.extras.is_empty() {
            1
        } else {
            STORE_SCHEMA
        },
        panels: contents.panels.clone(),
        overrides: contents.overrides.clone(),
        extras: contents.extras.clone(),
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
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let panels = &current.panels;
    let existing = panels.iter().position(|candidate| candidate.id == panel.id);
    // The store owns the version, so the editor may send any value.
    panel.version = existing.map_or(1, |index| panels[index].version.saturating_add(1));
    panel.validate()?;
    if existing.is_none() && panels.len() >= MAX_PANELS {
        return Err(format!("No more than {MAX_PANELS} panels can be saved."));
    }
    let mut next = current.clone();
    match existing {
        Some(index) => next.panels[index] = panel,
        None => next.panels.push(panel),
    }
    validate_set(&next.panels)?;
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    let _ = app.emit(EVENT, ());
    Ok(list(app))
}

pub fn delete(app: &AppHandle, id: &str) -> Result<Vec<CustomPanel>, String> {
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !current.panels.iter().any(|panel| panel.id == id) {
        return Err("That panel no longer exists.".into());
    }
    let mut next = current.clone();
    next.panels.retain(|panel| panel.id != id);
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    let _ = app.emit(EVENT, ());
    Ok(list(app))
}

/// Replaces every panel with a synced set that `plan_replacement` prepared.
/// Settings sync announces the change itself once the whole apply is done.
pub fn replace(app: &AppHandle, panels: Vec<CustomPanel>) -> Result<(), String> {
    validate_set(&panels)?;
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let next = Contents {
        panels,
        ..current.clone()
    };
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    Ok(())
}

/// Saves a rearranged built-in menu. The version rises above the saved
/// copy's, as for a panel.
pub fn save_override(
    app: &AppHandle,
    kind: crate::scan_menu::Kind,
    panel: CustomPanel,
) -> Result<(), String> {
    let mut panel = panel.normalized();
    let key = crate::builtin_panels::key(kind);
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    panel.version = current
        .overrides
        .get(&key)
        .map_or(1, |saved| saved.version.saturating_add(1));
    crate::builtin_panels::validate_override(kind, &panel)?;
    let mut next = current.clone();
    next.overrides.insert(key, panel);
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    let _ = app.emit(EVENT, ());
    Ok(())
}

/// Puts a built-in menu back as shipped.
pub fn reset_override(app: &AppHandle, kind: crate::scan_menu::Kind) -> Result<(), String> {
    let key = crate::builtin_panels::key(kind);
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !current.overrides.contains_key(&key) {
        return Ok(());
    }
    let mut next = current.clone();
    next.overrides.remove(&key);
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    let _ = app.emit(EVENT, ());
    Ok(())
}

/// Replaces every rearranged menu with a synced set. Settings sync
/// announces the change itself once the whole apply is done.
pub fn replace_overrides(
    app: &AppHandle,
    overrides: BTreeMap<String, CustomPanel>,
) -> Result<(), String> {
    crate::builtin_panels::validate_overrides(&overrides)?;
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let next = Contents {
        overrides,
        ..current.clone()
    };
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    Ok(())
}

/// Saves the extra tiles of one native panel. None takes the panel back to
/// its keys alone.
pub fn save_extras(
    app: &AppHandle,
    target: crate::panel_extras::Target,
    extras: crate::panel_extras::PanelExtras,
) -> Result<(), String> {
    let extras = extras.normalized();
    extras.validate()?;
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let mut next = current.clone();
    if extras.is_empty() {
        next.extras.remove(target.key());
    } else {
        next.extras.insert(target.key().into(), extras);
    }
    if next == *current {
        return Ok(());
    }
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    let _ = app.emit(EVENT, ());
    Ok(())
}

/// Replaces the extra tiles of every native panel with a synced set.
/// Settings sync announces the change itself once the whole apply is done.
pub fn replace_extras(app: &AppHandle, extras: crate::panel_extras::Extras) -> Result<(), String> {
    crate::panel_extras::validate_all(&extras)?;
    let controller = app.state::<Controller>();
    let mut current = controller
        .contents
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let next = Contents {
        extras,
        ..current.clone()
    };
    write(app, &next)?;
    *current = next;
    drop(current);
    crate::scanning_runtime::refresh_custom_panels(app);
    Ok(())
}

/// Prepares synced overrides to replace `local`: validates them and keeps
/// versions moving forward. Returns `None` when nothing would change.
pub fn plan_override_replacement(
    local: &BTreeMap<String, CustomPanel>,
    incoming: BTreeMap<String, CustomPanel>,
) -> Result<Option<BTreeMap<String, CustomPanel>>, String> {
    let mut incoming: BTreeMap<String, CustomPanel> = incoming
        .into_iter()
        .map(|(key, panel)| (key, panel.normalized()))
        .collect();
    crate::builtin_panels::validate_overrides(&incoming)?;
    for (key, panel) in &mut incoming {
        if let Some(existing) = local.get(key) {
            panel.version = if existing.same_content(panel) {
                existing.version.max(panel.version)
            } else {
                panel.version.max(existing.version.saturating_add(1))
            };
        }
    }
    if incoming == *local {
        return Ok(None);
    }
    Ok(Some(incoming))
}

/// Three-way merge of rearranged menus, one menu at a time: a menu changed
/// on one side wins, changed on both the account wins, and an edit is never
/// lost to a reset on the other side.
pub fn merge_overrides(
    base_local: &BTreeMap<String, CustomPanel>,
    base_cloud: &BTreeMap<String, CustomPanel>,
    local: &BTreeMap<String, CustomPanel>,
    cloud: &BTreeMap<String, CustomPanel>,
) -> BTreeMap<String, CustomPanel> {
    let keys: std::collections::BTreeSet<&String> = local.keys().chain(cloud.keys()).collect();
    let changed = |a: Option<&CustomPanel>, b: Option<&CustomPanel>| match (a, b) {
        (Some(a), Some(b)) => !a.same_content(b),
        (None, None) => false,
        _ => true,
    };
    keys.into_iter()
        .filter_map(|key| {
            let (l, c) = (local.get(key), cloud.get(key));
            let local_changed = changed(l, base_local.get(key));
            let cloud_changed = changed(c, base_cloud.get(key));
            let mut chosen = match (local_changed, cloud_changed) {
                (false, _) => c.cloned(),
                (true, false) => l.cloned(),
                (true, true) => c.or(l).cloned(),
            }?;
            chosen.version = [l, c]
                .into_iter()
                .flatten()
                .map(|p| p.version)
                .max()
                .unwrap_or(chosen.version);
            Some((key.clone(), chosen))
        })
        .collect()
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
        if !seen.insert(panel.name.trim().to_lowercase()) {
            let base: String = panel.name.chars().take(MAX_NAME_CHARS - 6).collect();
            let name = (2..)
                .map(|n| format!("{base} ({n})"))
                .find(|candidate| !seen.contains(&candidate.trim().to_lowercase()))
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
                    span: 1,
                },
                Cell {
                    label: "Hello".into(),
                    action: PanelAction::Text {
                        text: "Hello there".into(),
                    },
                    span: 2,
                },
            ],
            vec![Cell {
                label: "Back".into(),
                action: PanelAction::Back,
                span: 1,
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
                        {"label": "Copy", "action": {"type": "command", "command": "copy"}, "span": 1},
                        {"label": "Hello", "action": {"type": "text", "text": "Hello there"}, "span": 2}
                    ],
                    [{"label": "Back", "action": {"type": "back"}, "span": 1}]
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
        panel.rows[0][0].span = 0;
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.rows[0][0].span = MAX_SPAN + 1;
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok\tTab");
        assert!(panel.validate().is_err());
        panel = sample("ok", "Ok");
        panel.description = "Line one\nline two".into();
        assert_eq!(panel.validate(), Ok(()));
        panel.description = "Bell\u{7}".into();
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
            overrides: BTreeMap::new(),
            extras: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(load(&good).unwrap().panels.len(), 1);
        // Without overrides the file is still written as version 1 (see `write`).
        let written = serde_json::to_value(Store {
            schema_version: 1,
            panels: vec![],
            overrides: BTreeMap::new(),
            extras: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(written["schemaVersion"], 1);
        assert_eq!(written["overrides"], serde_json::json!({}));
        // A version 1 file has no overrides and still loads.
        let old = br#"{"schemaVersion":1,"panels":[]}"#;
        assert_eq!(load(old).unwrap(), Contents::default());
        let newer = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA + 1,
            panels: vec![],
            overrides: BTreeMap::new(),
            extras: BTreeMap::new(),
        })
        .unwrap();
        assert!(load(&newer).is_none());
        let mut bad = sample("a", "A");
        bad.rows.clear();
        let invalid = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA,
            panels: vec![bad],
            overrides: BTreeMap::new(),
            extras: BTreeMap::new(),
        })
        .unwrap();
        assert!(load(&invalid).is_none());
        // An override that is not a menu, or lost its way out, refuses the file.
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "shutDown".into(),
            crate::builtin_panels::default_document(crate::scan_menu::Kind::ShutDown),
        );
        let stray = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA,
            panels: vec![],
            overrides,
            extras: BTreeMap::new(),
        })
        .unwrap();
        assert!(load(&stray).is_none());
        // Extra rows on a panel that takes none refuse the file too.
        let mut extras = BTreeMap::new();
        extras.insert(
            "home".into(),
            crate::panel_extras::PanelExtras::rows(sample("a", "A").rows),
        );
        let misplaced = serde_json::to_vec(&Store {
            schema_version: STORE_SCHEMA,
            panels: vec![],
            overrides: BTreeMap::new(),
            extras,
        })
        .unwrap();
        assert!(load(&misplaced).is_none());
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
    fn menu_overrides_merge_one_menu_at_a_time_and_keep_versions_forward() {
        use crate::builtin_panels::{default_document, key};
        use crate::scan_menu::Kind;
        let home = || default_document(Kind::Home);
        let mut moved = home();
        moved.rows.reverse();
        let one = |panel: CustomPanel| BTreeMap::from([(key(Kind::Home), panel)]);
        // Rearranged here, untouched on the account: ours wins.
        let merged = merge_overrides(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &one(moved.clone()),
            &BTreeMap::new(),
        );
        assert_eq!(merged, one(moved.clone()));
        // Reset on the account, untouched here: the reset wins.
        let merged = merge_overrides(
            &one(moved.clone()),
            &one(moved.clone()),
            &one(moved.clone()),
            &BTreeMap::new(),
        );
        assert!(merged.is_empty());
        // Reset on the account but edited here: the edit survives.
        let mut edited = moved.clone();
        edited.rows[0][0].label = "Mine".into();
        let merged = merge_overrides(
            &one(moved.clone()),
            &one(moved.clone()),
            &one(edited.clone()),
            &BTreeMap::new(),
        );
        assert_eq!(merged[&key(Kind::Home)].rows[0][0].label, "Mine");
        // Replacement planning: same content is no change; new content moves the version on.
        assert_eq!(
            plan_override_replacement(&one(moved.clone()), one(moved.clone())),
            Ok(None)
        );
        let mut local_v3 = moved.clone();
        local_v3.version = 3;
        let planned = plan_override_replacement(&one(local_v3), one(edited))
            .unwrap()
            .unwrap();
        assert_eq!(planned[&key(Kind::Home)].version, 4);
        assert!(plan_override_replacement(
            &BTreeMap::new(),
            BTreeMap::from([("nonsense".to_string(), home())])
        )
        .is_err());
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
