//! App-owned switch settings and the embedded USAHP capture lease.
use crate::switches::Settings;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use usahp_daemon::embedded::{EmbeddedBroker, Event, Mode, StopReason};
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureState {
    pub active: bool,
    pub key: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub settings: Settings,
    pub capture: CaptureState,
    pub supported: bool,
    pub error: Option<String>,
    pub escape_hold_ms: u64,
    pub unavailable_keys: Vec<String>,
}
struct Data {
    settings: Settings,
    capture: CaptureState,
    capture_generation: u64,
    error: Option<String>,
}
pub struct Controller {
    data: Mutex<Data>,
    broker: Mutex<EmbeddedBroker>,
}
fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("switch-settings.json"))
        .map_err(|e| e.to_string())
}
fn persist(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app)?;
    let parent = path
        .parent()
        .ok_or("Switch settings directory is unavailable.")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Existing storage helper uses atomic replacement for desktop state.
    crate::storage::AppStorage::write_switch_settings(&path, settings)
}
impl Controller {
    fn new(app: &AppHandle) -> Self {
        let load = || -> Result<Settings, String> {
            let path = path(app)?;
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let s: Settings = serde_json::from_slice(&bytes)
                        .map_err(|e| format!("Switch settings could not be read: {e}"))?;
                    s.validate()?;
                    Ok(s)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let legacy = path.with_file_name("point-scan.json");
                    match std::fs::read(legacy) {
                        Ok(bytes) => {
                            let config: crate::point_scan::Config =
                                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                            config.validate()?;
                            let settings = Settings::migrate(&config);
                            persist(app, &settings)?;
                            Ok(settings)
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            let settings = Settings::default();
                            persist(app, &settings)?;
                            Ok(settings)
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        };
        let (settings, error) = match load() {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        Self {
            data: Mutex::new(Data {
                settings,
                capture: CaptureState::default(),
                capture_generation: 0,
                error,
            }),
            broker: Mutex::new(EmbeddedBroker::new()),
        }
    }
    pub fn view(&self) -> View {
        let d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        View {
            settings: d.settings.clone(),
            capture: d.capture.clone(),
            supported: cfg!(any(target_os = "windows", target_os = "macos")),
            error: d.error.clone(),
            escape_hold_ms: d.settings.escape_ms(),
            unavailable_keys: d
                .settings
                .bindings
                .iter()
                .filter(|b| !usahp_daemon::embedded::supported_key(&b.key))
                .map(|b| b.key.clone())
                .collect(),
        }
    }
    pub fn generation(&self) -> u64 {
        self.broker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .status()
            .generation
    }
    pub fn settings(&self) -> Settings {
        self.view().settings
    }
    pub fn save(&self, app: &AppHandle, settings: Settings) -> Result<View, String> {
        settings.validate()?;
        if self
            .broker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .status()
            .mode
            != Mode::Off
        {
            return Err("Disable scanning and finish capture before changing switches.".into());
        }
        {
            let d = self.data.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(error) = &d.error {
                return Err(error.clone());
            }
        }
        persist(app, &settings)?;
        self.data.lock().unwrap_or_else(|p| p.into_inner()).settings = settings;
        self.publish(app);
        Ok(self.view())
    }
    pub fn enable(&self, automatic: bool) -> Result<(), String> {
        let view = self.view();
        if let Some(e) = view.error {
            return Err(e);
        }
        view.settings.validate_actions(automatic)?;
        let mappings = view
            .settings
            .bindings
            .iter()
            .map(|b| usahp_core::Mapping {
                id: b.id.clone(),
                switch_id: b.id.clone(),
                input: usahp_core::InputKind::Keyboard,
                code: b.key.clone(),
                device: None,
            })
            .collect::<Vec<_>>();
        let mut broker = self.broker.lock().unwrap_or_else(|p| p.into_inner());
        broker
            .configure(&mappings, view.settings.escape_ms())
            .map_err(|e| e.to_string())?;
        broker.enable().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn begin_capture(&self, app: &AppHandle) -> Result<View, String> {
        if let Some(e) = self.view().error {
            return Err(e);
        }
        let generation = self
            .broker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .learn()
            .map_err(|e| e.to_string())?;
        let mut d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        d.capture = CaptureState {
            active: true,
            ..CaptureState::default()
        };
        d.capture_generation = generation;
        drop(d);
        self.publish(app);
        Ok(self.view())
    }
    pub fn cancel_capture(&self, app: &AppHandle) {
        let mut d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        if !d.capture.active {
            return;
        }
        d.capture = CaptureState::default();
        drop(d);
        self.broker.lock().unwrap_or_else(|p| p.into_inner()).stop();
        self.publish(app);
    }
    pub fn stop(&self) {
        self.broker.lock().unwrap_or_else(|p| p.into_inner()).stop();
    }
    pub fn shutdown(&self) {
        self.broker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .shutdown();
    }
    pub fn poll(&self, app: &AppHandle) -> (Vec<Event>, u64, u64) {
        let broker = self.broker.lock().unwrap_or_else(|p| p.into_inner());
        let events = broker.drain();
        let now = broker.monotonic_ms();
        let generation = broker.status().generation;
        drop(broker);
        let mut d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let mut changed = false;
        for event in &events {
            match event {
                Event::Learned { generation, code }
                    if d.capture.active && *generation == d.capture_generation =>
                {
                    d.capture = CaptureState {
                        active: false,
                        key: Some(code.clone()),
                        error: None,
                    };
                    changed = true;
                }
                Event::Stopped { generation, reason }
                    if d.capture.active && *generation == d.capture_generation =>
                {
                    d.capture = CaptureState {
                        active: false,
                        key: None,
                        error: Some(stop_message(*reason).into()),
                    };
                    changed = true;
                }
                _ => {}
            }
        }
        drop(d);
        if changed {
            self.publish(app);
        }
        (events, now, generation)
    }
    fn publish(&self, app: &AppHandle) {
        let _ = app.emit("switches-changed", self.view());
    }
}
pub fn stop_message(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Disabled => "Switch capture stopped.",
        StopReason::Escape => "Escape pressed. Switch capture stopped.",
        StopReason::HoldEscape => "Emergency hold released switch control.",
        StopReason::HeartbeatTimeout => "Switch capture stopped because its heartbeat was missed.",
        StopReason::QueueOverflow => "Switch capture stopped because input could not be processed.",
        StopReason::CaptureLost => {
            "Switch capture was lost. Check input permission and enable scanning again."
        }
    }
}
pub fn install(app: &AppHandle) {
    app.manage(Controller::new(app));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(
                usahp_daemon::embedded::HEARTBEAT_INTERVAL_MS,
            ))
            .await;
            app.state::<Controller>()
                .broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .heartbeat();
        }
    });
}
