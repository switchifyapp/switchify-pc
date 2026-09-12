//! Shared desktop scan controller. Adapters supply configuration, environment and activation.
use crate::{
    scan_host::Host,
    scanning::{Action, Session, SwitchSettings, Technique, TICK_MS},
};
use serde::{de::DeserializeOwned, Serialize};
pub trait Adapter: Send + Sync + 'static {
    type Config: Clone + Default + Serialize + DeserializeOwned + Send + Sync;
    type Technique: Technique + Send;
    type Environment: Clone + Send;
    const EVENT: &'static str;
    const FILE: &'static str;
    fn validate(config: &Self::Config) -> Result<(), String>;
    fn switches(config: &Self::Config) -> SwitchSettings;
    fn create(
        app: &AppHandle,
        config: Self::Config,
    ) -> Result<(Self::Technique, Self::Environment), String>;
    fn validate_environment(
        app: &AppHandle,
        environment: Option<&Self::Environment>,
    ) -> Result<(), String>;
    fn prepare(app: &AppHandle) -> Result<(), String>;
    fn activate(
        app: &AppHandle,
        selection: <Self::Technique as Technique>::Selection,
    ) -> Result<(), String>;
}

use crate::{switch_gestures::Gestures, switch_runtime, switches::Settings};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};
use usahp_daemon::embedded::Event;

thread_local! {static HOST:RefCell<Option<Host>>=const{RefCell::new(None)}; static PROMPT:RefCell<Option<Host>>=const{RefCell::new(None)};}
pub struct Controller<A: Adapter> {
    enabled: AtomicBool,
    generation: AtomicU64,
    data: Mutex<Data<A>>,
}
struct Data<A: Adapter> {
    config: A::Config,
    engine: Option<Session<A::Technique>>,
    display: Option<A::Environment>,
    pressed: Gestures,
    switches: Settings,
    input_generation: u64,
    last_tick: Instant,
    message: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View<C, P> {
    pub config: C,
    pub enabled: bool,
    pub phase: P,
    pub paused: bool,
    pub message: String,
    pub supported: bool,
}
impl<A: Adapter> Controller<A> {
    pub fn new(app: &AppHandle) -> Self {
        let config = config_path::<A>(app)
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<A::Config>(&b).ok())
            .filter(|c| A::validate(c).is_ok())
            .unwrap_or_default();
        Self {
            enabled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            data: Mutex::new(Data {
                config,
                engine: None,
                display: None,
                pressed: Gestures::default(),
                switches: Settings::default(),
                input_generation: 0,
                last_tick: Instant::now(),
                message: "Scanning is off.".into(),
            }),
        }
    }
    pub fn view(&self) -> View<A::Config, <A::Technique as Technique>::Phase> {
        let d = self.data.lock().unwrap_or_else(|p| p.into_inner());
        View {
            config: d.config.clone(),
            enabled: self.enabled.load(Ordering::SeqCst),
            phase: d
                .engine
                .as_ref()
                .map_or_else(Default::default, |e| e.technique.phase()),
            paused: d.engine.as_ref().is_some_and(|e| e.paused()),
            message: d.message.clone(),
            supported: cfg!(any(target_os = "windows", target_os = "macos")),
        }
    }
}
fn config_path<A: Adapter>(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join(A::FILE))
        .map_err(|e| e.to_string())
}
fn publish<A: Adapter>(app: &AppHandle) {
    let _ = app.emit(A::EVENT, app.state::<Controller<A>>().view());
}
// One local technique owns the switch keys and overlay at a time.
struct ScanService {
    cancel: fn(&AppHandle),
}
pub fn cancel(app: &AppHandle) {
    if let Some(service) = app.try_state::<ScanService>() {
        (service.cancel)(app);
    }
}
fn cancel_for<A: Adapter>(app: &AppHandle) {
    let Some(c) = app.try_state::<Controller<A>>() else {
        return;
    };
    c.enabled.store(false, Ordering::SeqCst);
    let generation = c.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if handle
            .state::<Controller<A>>()
            .generation
            .load(Ordering::SeqCst)
            == generation
        {
            disable::<A>(&handle, "Scanning stopped.");
        }
    });
}
fn disable<A: Adapter>(app: &AppHandle, message: &str) {
    let c = app.state::<Controller<A>>();
    c.enabled.store(false, Ordering::SeqCst);
    c.generation.fetch_add(1, Ordering::SeqCst);
    {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.engine = None;
        d.display = None;
        d.pressed.cancel();
        d.message = message.into();
    }
    app.state::<switch_runtime::Controller>().stop();
    HOST.with(|host| {
        if let Some(host) = host.borrow_mut().as_mut() {
            host.hide();
        }
    });
    hide_prompt();
    publish::<A>(app);
}
pub fn configure<A: Adapter>(
    app: &AppHandle,
    config: A::Config,
    enabled: bool,
) -> Result<View<A::Config, <A::Technique as Technique>::Phase>, String> {
    A::validate(&config)?;
    let path = config_path::<A>(app)?;
    disable::<A>(app, "Scanning is off.");
    let generation = app
        .state::<Controller<A>>()
        .generation
        .load(Ordering::SeqCst);
    if enabled {
        A::prepare(app)?;
        HOST.with(|slot| {
            if slot.borrow().is_none() {
                *slot.borrow_mut() = Some(Host::new()?);
            }
            Ok::<_, String>(())
        })?;
        app.state::<switch_runtime::Controller>()
            .enable(A::switches(&config).automatic)?;
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
        disable::<A>(app, "Scanning settings could not be saved.");
        return Err(error);
    }
    let c = app.state::<Controller<A>>();
    {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.config = config;
        d.switches = app.state::<switch_runtime::Controller>().settings();
        d.pressed = Gestures::default();
        d.input_generation = app.state::<switch_runtime::Controller>().generation();
        d.message = if enabled {
            "Ready. Press the select switch to begin."
        } else {
            "Scanning is off."
        }
        .into();
    }
    if enabled && c.generation.load(Ordering::SeqCst) != generation {
        disable::<A>(app, "Scanning stopped while enabling.");
        return Err("Scanning was cancelled while enabling. Try again.".into());
    }
    c.enabled.store(enabled, Ordering::SeqCst);
    publish::<A>(app);
    Ok(c.view())
}
fn switch<A: Adapter>(app: &AppHandle, action: Action, input_generation: u64) {
    let c = app.state::<Controller<A>>();
    if !c.enabled.load(Ordering::SeqCst) {
        return;
    }
    if !app
        .state::<switch_runtime::Controller>()
        .active_generation(input_generation)
    {
        disable::<A>(app, "Switch capture stopped.");
        return;
    }
    if action == Action::Cancel {
        disable::<A>(app, "Scanning cancelled. Switch keys released.");
        return;
    }
    hide_prompt();
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        if d.engine.as_ref().is_none_or(|e| !e.active()) && action == Action::Select {
            let (engine, display) = A::create(app, d.config.clone())?;
            d.engine = Some(Session::new(engine, A::switches(&d.config).automatic));
            d.display = Some(display);
            A::prepare(app)?;
        }
        let point = d.engine.as_mut().and_then(|e| e.action(action));
        d.last_tick = Instant::now();
        let display = d.display.clone();
        drop(d);
        if let Some(point) = point {
            A::validate_environment(app, display.as_ref())?;
            HOST.with(|host| {
                if let Some(h) = host.borrow_mut().as_mut() {
                    h.hide();
                }
            });
            if c.enabled.load(Ordering::SeqCst)
                && app
                    .state::<switch_runtime::Controller>()
                    .active_generation(input_generation)
            {
                A::activate(app, point)?;
            }
        }
        render::<A>(app)
    })();
    if let Err(error) = result {
        disable::<A>(app, &error);
    } else {
        publish::<A>(app);
    }
}
fn render<A: Adapter>(app: &AppHandle) -> Result<(), String> {
    let c = app.state::<Controller<A>>();
    let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    let frame = d
        .engine
        .as_ref()
        .map_or_else(Default::default, Session::frame);
    HOST.with(|host| {
        if let Some(host) = host.borrow_mut().as_mut() {
            host.render(&frame.strips)
        } else {
            Ok(())
        }
    })
}
fn tick<A: Adapter>(app: &AppHandle) {
    let c = app.state::<Controller<A>>();
    let (events, now_ms, _) = app.state::<switch_runtime::Controller>().poll(app);
    for event in events {
        if !c.enabled.load(Ordering::SeqCst) {
            break;
        }
        match event {
            Event::Stopped { reason, .. } => {
                disable::<A>(app, switch_runtime::stop_message(reason));
                return;
            }
            Event::Switch {
                generation,
                switch_id,
                action,
                monotonic_ms,
            } => {
                let selected = {
                    let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
                    if generation != d.input_generation
                        || !app
                            .state::<switch_runtime::Controller>()
                            .active_generation(generation)
                    {
                        continue;
                    }
                    if action == usahp_core::Action::Pressed {
                        let settings = d.switches.clone();
                        d.pressed.pressed(&switch_id, monotonic_ms, &settings);
                        None
                    } else {
                        d.pressed.released(&switch_id, monotonic_ms)
                    }
                };
                if let Some(action) = selected {
                    switch::<A>(app, action, generation);
                }
            }
            _ => {}
        }
    }
    if !c.enabled.load(Ordering::SeqCst) {
        return;
    }
    if app.state::<crate::state::AppModel>().snapshot().bluetooth
        == crate::state::BluetoothState::Connected
    {
        disable::<A>(app, "Android connected. Local scanning stopped.");
        return;
    }
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        A::validate_environment(app, d.display.as_ref())?;
        let now = Instant::now();
        let elapsed = now.duration_since(d.last_tick).as_millis() as u64;
        d.last_tick = now;
        let held = d.pressed.held();
        let prompt = d.pressed.prompt(now_ms);
        if let Some(engine) = d.engine.as_mut() {
            engine.tick(elapsed, held);
        }
        drop(d);
        show_prompt(app, prompt.as_ref())?;
        render::<A>(app)
    })();
    if let Err(error) = result {
        disable::<A>(app, &error);
    }
}
pub fn install<A: Adapter>(app: &AppHandle) {
    app.manage(Controller::<A>::new(app));
    app.manage(ScanService {
        cancel: cancel_for::<A>,
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)).await;
            let handle = app.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            if app
                .run_on_main_thread(move || {
                    tick::<A>(&handle);
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

fn hide_prompt() {
    PROMPT.with(|p| {
        if let Some(host) = p.borrow_mut().as_mut() {
            host.hide();
        }
    });
}
fn show_prompt(
    app: &AppHandle,
    prompt: Option<&crate::switch_gestures::Prompt>,
) -> Result<(), String> {
    let Some(prompt) = prompt else {
        hide_prompt();
        return Ok(());
    };
    let (cursor, displays) = crate::display_navigation::displays(app).map_err(|e| e.message)?;
    let display = crate::display_navigation::current_display(cursor, &displays)
        .ok_or("No display for the switch prompt.")?;
    let scale = if cfg!(target_os = "windows") {
        display.scale_factor
    } else {
        1.0
    };
    let width = (720.0 * scale)
        .min(f64::from(display.width) - 32.0 * scale)
        .max(1.0);
    let rect = crate::scanning::Rect {
        x: f64::from(display.x) + (f64::from(display.width) - width) / 2.0,
        y: f64::from(display.y) + 20.0 * scale,
        width,
        height: 64.0 * scale,
    };
    PROMPT.with(|p| {
        let mut p = p.borrow_mut();
        if p.is_none() {
            *p = Some(Host::new()?);
        }
        p.as_mut().unwrap().prompt(
            &format!(
                "Release {} for {}",
                prompt.switch_name,
                prompt.action.label()
            ),
            rect,
            scale,
        )
    })
}
