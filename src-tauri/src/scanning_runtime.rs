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
    /// Pure check that the environment allows scanning, polled while it is off.
    fn ready(app: &AppHandle) -> Result<(), String>;
    /// Side effects needed right before scanning uses the desktop.
    fn prepare(app: &AppHandle) -> Result<(), String>;
    fn activate(
        app: &AppHandle,
        selection: <Self::Technique as Technique>::Selection,
    ) -> Result<(), String>;
}

use crate::switch_input::Event;
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

thread_local! {static HOST:RefCell<Option<Host>>=const{RefCell::new(None)}; static PROMPT:RefCell<Option<Host>>=const{RefCell::new(None)};}
/// Scanning has no on/off switch. It is armed whenever the saved switches can
/// drive the current mode and the environment allows it, and the tick loop
/// re-arms it after anything that stopped it: a save, key learning, Escape, an
/// Android session or a failed key reservation. Failed attempts back off by this
/// much so a key held by another application is not hammered every tick.
const RETRY_MS: u64 = 2000;
pub struct Controller<A: Adapter> {
    enabled: AtomicBool,
    halted: AtomicBool,
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
    next_attempt: Option<Instant>,
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
            halted: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            data: Mutex::new(Data {
                config,
                engine: None,
                display: None,
                pressed: Gestures::default(),
                switches: Settings::default(),
                input_generation: 0,
                last_tick: Instant::now(),
                message: "Starting switch scanning...".into(),
                next_attempt: None,
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
    halt: fn(&AppHandle),
}
/// Stops scanning now; the tick loop re-arms it once conditions allow.
pub fn cancel(app: &AppHandle) {
    if let Some(service) = app.try_state::<ScanService>() {
        (service.cancel)(app);
    }
}
/// Stops scanning for good, for application exit.
pub fn halt(app: &AppHandle) {
    if let Some(service) = app.try_state::<ScanService>() {
        (service.halt)(app);
    }
}
fn halt_for<A: Adapter>(app: &AppHandle) {
    if let Some(c) = app.try_state::<Controller<A>>() {
        c.halted.store(true, Ordering::SeqCst);
    }
    cancel_for::<A>(app);
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
/// Pauses scanning so switches can be saved or learned. Must run on the main
/// thread, as the commands that call it do; the tick loop re-arms afterwards.
pub fn pause<A: Adapter>(app: &AppHandle) {
    disable::<A>(app, "Scanning paused while switches change.");
}
/// Saves new settings and re-arms scanning with them. A failure to arm is not
/// an error here: the settings are saved and the view's message says why
/// scanning is off, and the tick loop keeps trying.
pub fn configure<A: Adapter>(
    app: &AppHandle,
    config: A::Config,
) -> Result<View<A::Config, <A::Technique as Technique>::Phase>, String> {
    A::validate(&config)?;
    let path = config_path::<A>(app)?;
    disable::<A>(app, "Applying scanning settings...");
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
        d.next_attempt = None;
    }
    ensure::<A>(app);
    publish::<A>(app);
    Ok(c.view())
}
/// Why scanning should stay off, if it should. Pure, so it is safe every tick.
fn wanted<A: Adapter>(app: &AppHandle, config: &A::Config) -> Result<(), String> {
    if !cfg!(any(target_os = "windows", target_os = "macos")) {
        return Err("Switch scanning is not available on this platform.".into());
    }
    let switches = app.state::<switch_runtime::Controller>().view();
    if let Some(error) = switches.error {
        return Err(error);
    }
    if switches.capture.active {
        return Err("Learning a switch. Scanning resumes afterwards.".into());
    }
    switches
        .settings
        .validate_actions(A::switches(config).automatic)?;
    A::ready(app)
}
/// Reserves the switch keys and readies the overlay. Leaves nothing behind on
/// failure: the broker only enables after every key registered. Only failures
/// here start the retry backoff; environment conditions are re-read each tick.
fn arm<A: Adapter>(app: &AppHandle, config: &A::Config) -> Result<(), String> {
    A::prepare(app)?;
    HOST.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = Some(Host::new()?);
        }
        Ok::<_, String>(())
    })?;
    let switches = app.state::<switch_runtime::Controller>();
    switches.enable(A::switches(config).automatic)?;
    let c = app.state::<Controller<A>>();
    let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    d.engine = None;
    d.display = None;
    d.switches = switches.settings();
    d.pressed = Gestures::default();
    d.input_generation = switches.generation();
    d.last_tick = Instant::now();
    Ok(())
}
/// Arms scanning if it is wanted and not already running. Main thread only.
fn ensure<A: Adapter>(app: &AppHandle) {
    let c = app.state::<Controller<A>>();
    if c.enabled.load(Ordering::SeqCst) || c.halted.load(Ordering::SeqCst) {
        return;
    }
    let (config, due) = {
        let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        (
            d.config.clone(),
            d.next_attempt.is_none_or(|at| Instant::now() >= at),
        )
    };
    let outcome = match wanted::<A>(app, &config) {
        Err(reason) => Err((reason, false)),
        Ok(()) if !due => return,
        Ok(()) => arm::<A>(app, &config).map_err(|e| (e, true)),
    };
    let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    match outcome {
        Ok(()) => {
            d.next_attempt = None;
            d.message = "Ready. Press the select switch to begin.".into();
            drop(d);
            c.enabled.store(true, Ordering::SeqCst);
            publish::<A>(app);
        }
        Err((message, failed)) => {
            if failed {
                d.next_attempt = Some(Instant::now() + std::time::Duration::from_millis(RETRY_MS));
            }
            let changed = d.message != message;
            d.message = message;
            drop(d);
            if changed {
                publish::<A>(app);
            }
        }
    }
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
        // Escape resets the scan; the next tick re-arms the keys.
        disable::<A>(app, "Escape pressed. Scanning reset.");
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
                    if action == crate::switch_input::Action::Pressed {
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
        ensure::<A>(app);
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
        halt: halt_for::<A>,
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
