use crate::{
    display_navigation::{self, Display},
    point_scan::{Action, Config, Engine, Phase, Rect, ACTIONS},
    point_scan_host::Host,
};
use serde::Serialize;
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

thread_local! {static HOST:RefCell<Option<Host>>=const{RefCell::new(None)};}
pub struct Controller {
    enabled: AtomicBool,
    generation: AtomicU64,
    data: Mutex<Data>,
}
struct Data {
    config: Config,
    engine: Option<Engine>,
    display: Option<Display>,
    registered: Vec<String>,
    pressed: [bool; 5],
    last_tick: Instant,
    message: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub config: Config,
    pub enabled: bool,
    pub phase: Phase,
    pub paused: bool,
    pub message: String,
    pub supported: bool,
}
impl Controller {
    pub fn new(app: &AppHandle) -> Self {
        let config = config_path(app)
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .filter(|c| c.validate().is_ok())
            .unwrap_or_default();
        Self {
            enabled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            data: Mutex::new(Data {
                config,
                engine: None,
                display: None,
                registered: vec![],
                pressed: [false; 5],
                last_tick: Instant::now(),
                message: "Point scan is off.".into(),
            }),
        }
    }
    pub fn view(&self) -> View {
        let d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        View {
            config: d.config.clone(),
            enabled: self.enabled.load(Ordering::SeqCst),
            phase: d.engine.as_ref().map_or(Phase::Idle, |e| e.phase),
            paused: d.engine.as_ref().is_some_and(|e| e.paused),
            message: d.message.clone(),
            supported: cfg!(any(target_os = "windows", target_os = "macos")),
        }
    }
}
fn config_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("point-scan.json"))
        .map_err(|e| e.to_string())
}
fn publish(app: &AppHandle) {
    let _ = app.emit("point-scan-changed", app.state::<Controller>().view());
}
/// Can be called from transport threads while their input lock is held.
pub fn cancel(app: &AppHandle) {
    if let Some(c) = app.try_state::<Controller>() {
        c.enabled.store(false, Ordering::SeqCst);
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || disable(&handle, "Point scan stopped."));
}
fn disable(app: &AppHandle, message: &str) {
    let c = app.state::<Controller>();
    c.enabled.store(false, Ordering::SeqCst);
    c.generation.fetch_add(1, Ordering::SeqCst);
    let keys = {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.engine = None;
        d.display = None;
        d.pressed = [false; 5];
        d.message = message.into();
        std::mem::take(&mut d.registered)
    };
    for key in keys {
        let _ = app.global_shortcut().unregister(key.as_str());
    }
    HOST.with(|host| {
        if let Some(host) = host.borrow_mut().as_mut() {
            host.hide();
        }
    });
    publish(app);
}
pub fn configure(app: &AppHandle, config: Config, enabled: bool) -> Result<View, String> {
    config.validate()?;
    let path = config_path(app)?;
    disable(app, "Point scan is off.");
    if enabled {
        crate::point_scan_prepare(app)?;
        HOST.with(|slot| {
            if slot.borrow().is_none() {
                *slot.borrow_mut() = Some(Host::new()?);
            }
            Ok::<_, String>(())
        })?;
        let generation = app.state::<Controller>().generation.load(Ordering::SeqCst);
        for (index, key) in config.keys().iter().enumerate() {
            if let Err(error) = app
                .global_shortcut()
                .on_shortcut(*key, move |app, _, event| {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if handle
                            .state::<Controller>()
                            .generation
                            .load(Ordering::SeqCst)
                            == generation
                        {
                            switch(&handle, index, event.state == ShortcutState::Pressed);
                        }
                    });
                })
            {
                disable(app, "A switch key is already in use. Choose another key.");
                return Err(format!("Could not reserve {key}: {error}"));
            }
            app.state::<Controller>()
                .data
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .registered
                .push((*key).into());
        }
    }
    let save = || -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    };
    if let Err(error) = save() {
        disable(app, "Point scan settings could not be saved.");
        return Err(error);
    }
    let c = app.state::<Controller>();
    {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.config = config;
        d.message = if enabled {
            "Ready. Press the select switch to begin."
        } else {
            "Point scan is off."
        }
        .into();
    }
    c.enabled.store(enabled, Ordering::SeqCst);
    publish(app);
    Ok(c.view())
}
fn new_engine(app: &AppHandle, config: Config) -> Result<(Engine, Display), String> {
    let (cursor, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
    let display = display_navigation::current_display(cursor, &displays)
        .ok_or("No scanning display is available.")?
        .clone();
    let units = if cfg!(target_os = "windows") {
        display.scale_factor
    } else {
        1.0
    };
    let e = Engine::new(
        config,
        Rect {
            x: display.x.into(),
            y: display.y.into(),
            width: display.width.into(),
            height: display.height.into(),
        },
        units,
    )?;
    Ok((e, display))
}
fn switch(app: &AppHandle, index: usize, pressed: bool) {
    let c = app.state::<Controller>();
    if !c.enabled.load(Ordering::SeqCst) {
        return;
    }
    if ACTIONS[index] == Action::Cancel {
        if pressed {
            disable(app, "Point scan cancelled. Switch keys released.");
        }
        return;
    }
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        if pressed {
            d.pressed[index] = true;
            return Ok(());
        }
        if !std::mem::replace(&mut d.pressed[index], false) {
            return Ok(());
        }
        if d.engine.as_ref().is_none_or(|e| e.phase == Phase::Idle)
            && ACTIONS[index] == Action::Select
        {
            let (engine, display) = new_engine(app, d.config.clone())?;
            d.engine = Some(engine);
            d.display = Some(display);
            crate::point_scan_prepare(app)?;
        }
        let point = d.engine.as_mut().and_then(|e| e.action(ACTIONS[index]));
        d.last_tick = Instant::now();
        let display = d.display.clone();
        drop(d);
        if let Some(point) = point {
            validate_display(app, display.as_ref())?;
            HOST.with(|host| {
                if let Some(h) = host.borrow_mut().as_mut() {
                    h.hide();
                }
            });
            if c.enabled.load(Ordering::SeqCst) {
                crate::point_scan_click(app, point)?;
            }
        }
        render(app)
    })();
    if let Err(error) = result {
        disable(app, &error);
    } else {
        publish(app);
    }
}
fn validate_display(app: &AppHandle, display: Option<&Display>) -> Result<(), String> {
    if let Some(expected) = display {
        let (_, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
        if !displays.contains(expected) {
            return Err("Display geometry changed. Enable point scan again.".into());
        }
    }
    Ok(())
}
fn render(app: &AppHandle) -> Result<(), String> {
    let c = app.state::<Controller>();
    let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    let lines = d.engine.as_ref().map_or_else(Vec::new, Engine::lines);
    HOST.with(|host| {
        if let Some(host) = host.borrow_mut().as_mut() {
            host.render(&lines)
        } else {
            Ok(())
        }
    })
}
fn tick(app: &AppHandle) {
    let c = app.state::<Controller>();
    if !c.enabled.load(Ordering::SeqCst) {
        return;
    }
    if app.state::<crate::state::AppModel>().snapshot().bluetooth
        == crate::state::BluetoothState::Connected
    {
        disable(app, "Android connected. Local point scan stopped.");
        return;
    }
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        validate_display(app, d.display.as_ref())?;
        let now = Instant::now();
        let elapsed = now.duration_since(d.last_tick).as_millis() as u64;
        d.last_tick = now;
        if !d.pressed[0] {
            if let Some(engine) = d.engine.as_mut() {
                engine.tick(elapsed);
            }
        }
        drop(d);
        render(app)
    })();
    if let Err(error) = result {
        disable(app, &error);
    }
}
pub fn install(app: &AppHandle) {
    app.manage(Controller::new(app));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(33)).await;
            let handle = app.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            if app
                .run_on_main_thread(move || {
                    tick(&handle);
                    let _ = tx.send(());
                })
                .is_err()
                || rx.await.is_err()
            {
                break;
            }
        }
    });
}
