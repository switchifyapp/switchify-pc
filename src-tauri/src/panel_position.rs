//! The scanning panel position chosen on this computer. It is local state,
//! kept apart from the synced scanner settings so their format is unchanged.
use crate::scan_panel::Dock;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const FILE: &str = "panel-position.json";

pub fn path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(FILE))
}

/// The saved position, or the default when none is saved or it cannot be used.
pub fn load(path: &Path) -> Dock {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Dock>(&bytes).ok())
        .filter(|dock| dock.column <= 2 && dock.row <= 2)
        .unwrap_or_default()
}

/// Replaces the file whole so an interrupted save never leaves it truncated.
pub fn save(path: &Path, dock: Dock) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec(&dock).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    let saved = (|| {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()
    })()
    .map_err(|e| e.to_string())
    .and_then(|()| crate::storage::replace_file(&temp, path));
    if saved.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "switchify-panel-position-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_saved_position_loads_back() {
        let dir = temp_dir("round-trip");
        let path = dir.join(FILE);
        let dock = Dock { column: 0, row: 0 };
        save(&path, dock).unwrap();
        assert_eq!(load(&path), dock);
        let dock = Dock { column: 2, row: 1 };
        save(&path, dock).unwrap();
        assert_eq!(load(&path), dock);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"column":2,"row":1}"#
        );
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_or_invalid_positions_use_the_default() {
        let dir = temp_dir("invalid");
        let path = dir.join(FILE);
        assert_eq!(load(&path), Dock::default(), "missing file");
        std::fs::create_dir_all(&dir).unwrap();
        for contents in [
            "",
            "not json",
            "{}",
            r#"{"column":3,"row":0}"#,
            r#"{"column":0,"row":9}"#,
            r#"{"column":-1,"row":0}"#,
            r#"{"column":"left","row":0}"#,
        ] {
            std::fs::write(&path, contents).unwrap();
            assert_eq!(load(&path), Dock::default(), "{contents}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
