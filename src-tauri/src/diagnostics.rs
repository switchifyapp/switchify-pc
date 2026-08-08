use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::state::{AccessibilityState, ActivityKind, AppState, BluetoothState};
use crate::storage::replace_file;

const MAX_EVENTS: usize = 500;
const SUMMARY_EVENTS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub sequence: u64,
    pub timestamp: i64,
    pub category: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummary {
    pub recent_bluetooth: Vec<DiagnosticEvent>,
    pub last_disconnect: Option<DiagnosticEvent>,
    pub recent_errors: Vec<DiagnosticEvent>,
}

#[derive(Debug)]
struct HistoryData {
    events: VecDeque<DiagnosticEvent>,
    next_sequence: u64,
    last_bluetooth: Option<BluetoothState>,
    last_accessibility: Option<AccessibilityState>,
    last_error: Option<String>,
}

#[derive(Debug)]
pub struct DiagnosticHistory {
    path: PathBuf,
    data: Mutex<HistoryData>,
}

impl DiagnosticHistory {
    pub fn new(path: PathBuf) -> Self {
        let events = load_events(&path);
        let next_sequence = events.back().map_or(1, |event| event.sequence + 1);
        Self {
            path,
            data: Mutex::new(HistoryData {
                events,
                next_sequence,
                last_bluetooth: None,
                last_accessibility: None,
                last_error: None,
            }),
        }
    }

    pub fn startup(&self, timestamp: i64) -> DiagnosticSummary {
        self.record(timestamp, "lifecycle", "started", None)
    }

    #[cfg(test)]
    fn disconnect(&self, timestamp: i64, reason: &str) -> DiagnosticSummary {
        self.record(
            timestamp,
            "disconnect",
            "disconnected",
            Some(sanitize_detail(reason)),
        )
    }

    pub fn updater(&self, timestamp: i64, status: &str, detail: Option<&str>) -> DiagnosticSummary {
        self.record(timestamp, "updater", status, detail.map(sanitize_detail))
    }

    pub fn observe(&self, state: &AppState, timestamp: i64) -> DiagnosticSummary {
        let mut pending = Vec::new();
        {
            let mut data = self
                .data
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if data.last_bluetooth != Some(state.bluetooth) {
                if data.last_bluetooth == Some(BluetoothState::Connected)
                    && state.bluetooth != BluetoothState::Connected
                {
                    pending.push((
                        "disconnect",
                        "disconnected",
                        Some("Bluetooth session ended.".to_string()),
                    ));
                }
                data.last_bluetooth = Some(state.bluetooth);
                pending.push(("bluetooth", bluetooth_status(state.bluetooth), None));
            }
            if data.last_accessibility != Some(state.accessibility) {
                data.last_accessibility = Some(state.accessibility);
                pending.push((
                    "accessibility",
                    accessibility_status(state.accessibility),
                    None,
                ));
            }
            if let Some(activity) = state
                .last_activity
                .as_ref()
                .filter(|activity| activity.kind == ActivityKind::Error)
            {
                let sanitized = sanitize_detail(&activity.message);
                if data.last_error.as_deref() != Some(&sanitized) {
                    data.last_error = Some(sanitized.clone());
                    pending.push(("runtime", "failed", Some(sanitized)));
                }
            }
        }
        for (category, status, detail) in pending {
            self.record(timestamp, category, status, detail);
        }
        self.summary()
    }

    pub fn events(&self) -> Vec<DiagnosticEvent> {
        self.data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .events
            .iter()
            .cloned()
            .collect()
    }

    pub fn summary(&self) -> DiagnosticSummary {
        let data = self
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        summarize(&data.events)
    }

    fn record(
        &self,
        timestamp: i64,
        category: &str,
        status: &str,
        detail: Option<String>,
    ) -> DiagnosticSummary {
        let mut data = self
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let event = DiagnosticEvent {
            sequence: data.next_sequence,
            timestamp,
            category: category.to_owned(),
            status: status.to_owned(),
            detail,
        };
        data.next_sequence += 1;
        data.events.push_back(event);
        while data.events.len() > MAX_EVENTS {
            data.events.pop_front();
        }
        // Diagnostic storage must never prevent the application from running.
        let _ = write_events(&self.path, &data.events);
        summarize(&data.events)
    }
}

fn bluetooth_status(state: BluetoothState) -> &'static str {
    match state {
        BluetoothState::Initializing => "initializing",
        BluetoothState::Advertising => "advertising",
        BluetoothState::Connected => "connected",
        BluetoothState::PoweredOff => "poweredOff",
        BluetoothState::Unauthorized => "unauthorized",
        #[cfg(target_os = "windows")]
        BluetoothState::Conflict => "conflict",
        BluetoothState::Unsupported => "unsupported",
        BluetoothState::Error => "error",
    }
}

fn accessibility_status(state: AccessibilityState) -> &'static str {
    match state {
        AccessibilityState::Granted => "granted",
        AccessibilityState::Required => "required",
        AccessibilityState::Unavailable => "unavailable",
    }
}

fn summarize(events: &VecDeque<DiagnosticEvent>) -> DiagnosticSummary {
    let recent = |category: &str| {
        let mut matching = events
            .iter()
            .rev()
            .filter(|event| event.category == category)
            .take(SUMMARY_EVENTS)
            .cloned()
            .collect::<Vec<_>>();
        matching.reverse();
        matching
    };
    DiagnosticSummary {
        recent_bluetooth: recent("bluetooth"),
        last_disconnect: events
            .iter()
            .rev()
            .find(|event| event.category == "disconnect")
            .cloned(),
        recent_errors: events
            .iter()
            .rev()
            .filter(|event| event.status == "failed" || event.status == "error")
            .take(SUMMARY_EVENTS)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
    }
}

fn load_events(path: &Path) -> VecDeque<DiagnosticEvent> {
    let Ok(raw) = fs::read_to_string(path) else {
        return VecDeque::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<DiagnosticEvent>(line).ok())
        .rev()
        .take(MAX_EVENTS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn write_events(path: &Path, events: &VecDeque<DiagnosticEvent>) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Diagnostic history directory is unavailable.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut contents = String::new();
    for event in events {
        contents.push_str(&serde_json::to_string(event).map_err(|error| error.to_string())?);
        contents.push('\n');
    }
    let temp = path.with_extension("jsonl.tmp");
    fs::write(&temp, contents).map_err(|error| error.to_string())?;
    replace_file(&temp, path)
}

pub(crate) fn sanitize_detail(value: &str) -> String {
    let mut words = Vec::new();
    for word in value.split_whitespace().take(30) {
        let lower = word.to_ascii_lowercase();
        let looks_sensitive = word.contains('/')
            || word.contains('\\')
            || word.contains('@')
            || word.contains('{')
            || word.contains('}')
            || lower.contains("token")
            || lower.contains("nonce")
            || lower.contains("signature")
            || lower.contains("password")
            || lower.contains("secret")
            || lower.contains("pairing")
            || lower.contains("bearer")
            || lower.contains("api_key")
            || lower.starts_with("tb_")
            || (word.len() >= 24
                && word.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "-_=.".contains(character)
                }));
        words.push(if looks_sensitive { "[redacted]" } else { word });
    }
    let sanitized = words.join(" ");
    if sanitized.is_empty() {
        "No additional details.".into()
    } else {
        sanitized.chars().take(240).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("switchify-diagnostics-{}", uuid::Uuid::new_v4()))
            .join("diagnostic-history.jsonl")
    }

    #[test]
    fn history_is_bounded_ordered_and_survives_restart() {
        let path = temp_path();
        let history = DiagnosticHistory::new(path.clone());
        for timestamp in 0..(MAX_EVENTS as i64 + 7) {
            history.disconnect(timestamp, "session ended");
        }
        let restored = DiagnosticHistory::new(path.clone()).events();
        assert_eq!(restored.len(), MAX_EVENTS);
        assert_eq!(restored.first().unwrap().timestamp, 7);
        assert_eq!(restored.last().unwrap().timestamp, 506);
        assert!(restored
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn malformed_lines_are_ignored() {
        let path = temp_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json\n{\"sequence\":4,\"timestamp\":5,\"category\":\"runtime\",\"status\":\"failed\"}\n").unwrap();
        let events = DiagnosticHistory::new(path.clone()).events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 4);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn details_remove_paths_credentials_and_long_identifiers() {
        let sanitized = sanitize_detail("failed /Users/person/private token=secret user@example.com ABCDEFGHIJKLMNOPQRSTUVWXYZ1234 safe");
        assert_eq!(
            sanitized,
            "failed [redacted] [redacted] [redacted] [redacted] safe"
        );
    }

    #[test]
    fn unwritable_history_is_best_effort() {
        let root = std::env::temp_dir().join(format!("switchify-file-{}", uuid::Uuid::new_v4()));
        fs::write(&root, "not a directory").unwrap();
        let history = DiagnosticHistory::new(root.join("history.jsonl"));
        let summary = history.disconnect(1, "manual request");
        assert_eq!(summary.last_disconnect.unwrap().status, "disconnected");
        let _ = fs::remove_file(root);
    }

    #[test]
    fn connected_to_advertising_records_a_disconnect_without_device_data() {
        let path = temp_path();
        let history = DiagnosticHistory::new(path.clone());
        let mut state = crate::state::AppModel::with_storage_for_test(
            crate::storage::AppStorage::at(path.with_file_name("state.json")),
        )
        .snapshot();
        state.bluetooth = BluetoothState::Connected;
        history.observe(&state, 1);
        state.bluetooth = BluetoothState::Advertising;
        let summary = history.observe(&state, 2);
        let disconnect = summary.last_disconnect.unwrap();
        assert_eq!(disconnect.timestamp, 2);
        assert_eq!(
            disconnect.detail.as_deref(),
            Some("Bluetooth session ended.")
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
