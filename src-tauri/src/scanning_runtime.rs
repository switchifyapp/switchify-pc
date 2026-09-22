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
    fn deferred(_request: &<Self::Technique as Technique>::Selection) -> bool {
        false
    }
    fn preserve_visuals(_request: &<Self::Technique as Technique>::Selection) -> bool {
        false
    }
    fn poll(_app: &AppHandle, _technique: &mut Self::Technique, _captured_keys: &[String]) {}
    fn cleanup(app: &AppHandle) -> Result<(), String>;
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
    ) -> Result<Option<Self::Environment>, String>;
    fn settle_environment(
        app: &AppHandle,
        environment: Option<&mut Self::Environment>,
        technique: Option<&mut Self::Technique>,
    ) -> Result<bool, String>;
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

thread_local! {static COUNTDOWN: RefCell<(Option<Host>, Option<crate::scanning::Countdown>)> = const { RefCell::new((None, None)) }; static HOST:RefCell<Option<Host>>=const{RefCell::new(None)}; static PROMPT:RefCell<Option<Host>>=const{RefCell::new(None)}; static LABEL:RefCell<Option<Host>>=const{RefCell::new(None)}; static TILES:RefCell<(Vec<Host>,Vec<crate::scanning::FrameTile>)>=const{RefCell::new((vec![],vec![]))};}
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
    cursor_suppression: Option<(u64, Instant)>,
    config: A::Config,
    engine: Option<Session<A::Technique>>,
    display: Option<A::Environment>,
    pressed: Gestures,
    switches: Settings,
    input_generation: u64,
    remote: bool,
    remote_hold_started: Option<u64>,
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
    pub remote: bool,
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
                cursor_suppression: None,
                config,
                engine: None,
                display: None,
                pressed: Gestures::default(),
                switches: Settings::default(),
                input_generation: 0,
                remote: false,
                remote_hold_started: None,
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
            remote: d.remote,
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
    crate::remote_scan::cancel(app);
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
    crate::remote_scan::cancel(app);
    reset_scanner::<A>(app, message);
}
fn reset_scanner<A: Adapter>(app: &AppHandle, message: &str) {
    reset_scanner_for_capture::<A>(app, message, false);
}
fn reset_scanner_for_capture<A: Adapter>(app: &AppHandle, message: &str, recovering: bool) {
    let c = app.state::<Controller<A>>();
    c.enabled.store(false, Ordering::SeqCst);
    c.generation.fetch_add(1, Ordering::SeqCst);
    {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.engine = None;
        d.display = None;
        d.pressed.cancel();
        d.remote = false;
        d.remote_hold_started = None;
        d.message = message.into();
    }
    let cleanup = A::cleanup(app);
    crate::remote_scan::record_cleanup(app, cleanup.is_ok());
    if cleanup.is_err() {
        c.data.lock().unwrap_or_else(|p| p.into_inner()).message =
            "Input cleanup will be retried before scanning resumes.".into();
    }
    if recovering {
        app.state::<switch_runtime::Controller>()
            .stop_for_recovery();
    } else {
        app.state::<switch_runtime::Controller>().stop();
    }
    hide_scan_visuals();
    let token = c
        .data
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .cursor_suppression
        .take();
    if let Some((token, _)) = token {
        app.state::<crate::overlay::CursorOverlay>()
            .release_scan(token);
    }
    publish::<A>(app);
}
/// Pauses scanning so settings can be saved. Must run on the main thread, as
/// the commands that call it do. A live remote session survives: only the
/// scanner and the local key broker reset, and the next tick restarts the
/// session with the freshly applied settings.
pub fn pause<A: Adapter>(app: &AppHandle) {
    reset_scanner::<A>(app, "Scanning paused while switches change.");
}
/// Stops scanning outright, ending any remote session, so the local key
/// broker is free for learning a key.
pub fn interrupt<A: Adapter>(app: &AppHandle) {
    disable::<A>(app, "Scanning paused while a switch is learned.");
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
    reset_scanner::<A>(app, "Applying scanning settings...");
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
fn prediction_captured_keys(remote: bool, settings: &Settings) -> Vec<String> {
    let mut keys = vec!["Escape".to_owned()];
    if !remote {
        keys.extend(settings.bindings.iter().map(|b| b.key.clone()));
    }
    keys
}
fn source_is_current(remote: bool, remote_current: bool, local_current: bool) -> bool {
    if remote {
        remote_current
    } else {
        local_current
    }
}
fn input_active(app: &AppHandle, generation: u64, remote: bool) -> bool {
    source_is_current(
        remote,
        crate::remote_scan::active_generation(app, generation),
        app.state::<switch_runtime::Controller>()
            .active_generation(generation),
    )
}
fn switch<A: Adapter>(app: &AppHandle, action: Action, input_generation: u64, remote: bool) {
    let c = app.state::<Controller<A>>();
    if !c.enabled.load(Ordering::SeqCst) {
        return;
    }
    if !input_active(app, input_generation, remote) {
        crate::remote_scan::cancel(app);
        reset_scanner_for_capture::<A>(app, "Switch capture stopped.", true);
        return;
    }
    if matches!(action, Action::Cancel | Action::Stop) {
        // Escape resets the scan; the next tick re-arms the keys.
        disable::<A>(app, "Escape pressed. Scanning reset.");
        return;
    }
    // Do not act on a scan the user cannot see during the native handoff.
    if c.data
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .cursor_suppression
        .is_some_and(|(token, _)| {
            !app.state::<crate::overlay::CursorOverlay>()
                .scan_ready(token)
        })
    {
        return;
    }
    hide_prompt();
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        if action == Action::OpenKeyboard
            || (d.engine.as_ref().is_none_or(|e| !e.active()) && action == Action::Select)
        {
            let (engine, display) = A::create(app, d.config.clone())?;
            d.engine = Some(Session::new(engine, A::switches(&d.config).automatic));
            d.display = Some(display);
            A::prepare(app)?;
        }
        let Data {
            display, engine, ..
        } = &mut *d;
        if !A::settle_environment(
            app,
            display.as_mut(),
            engine.as_mut().map(|e| &mut e.technique),
        )? {
            return Ok(());
        }
        A::validate_environment(app, d.display.as_ref())?;
        let point = d.engine.as_mut().and_then(|e| e.action(action));
        d.last_tick = Instant::now();
        let display = d.display.clone();
        drop(d);
        if let Some(point) = point {
            dispatch::<A>(app, point, display.as_ref(), input_generation, remote)?;
        }
        render::<A>(app, None)
    })();
    if let Err(error) = result {
        disable::<A>(app, &error);
    } else {
        publish::<A>(app);
    }
}
fn dispatch<A: Adapter>(
    app: &AppHandle,
    request: <A::Technique as Technique>::Selection,
    environment: Option<&A::Environment>,
    input_generation: u64,
    remote: bool,
) -> Result<(), String> {
    A::validate_environment(app, environment)?;
    let c = app.state::<Controller<A>>();
    if !c.enabled.load(Ordering::SeqCst) || !input_active(app, input_generation, remote) {
        return Err("Scan action was cancelled.".into());
    }
    if !A::preserve_visuals(&request) {
        hide_scan_visuals();
    }
    let deferred = A::deferred(&request);
    match A::activate(app, request) {
        Ok(environment) => {
            let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(environment) = environment {
                d.display = Some(environment);
            }
            if let Some(engine) = d.engine.as_mut() {
                if !deferred {
                    engine.technique.execution_succeeded();
                }
            }
        }
        Err(error) => {
            A::cleanup(app)?;
            let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
            d.message = error.clone();
            if let Some(engine) = d.engine.as_mut() {
                engine.execution_failed(error);
            }
        }
    }
    {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        if d.engine.as_ref().is_some_and(|e| !e.active()) {
            d.display = None;
        }
    }
    Ok(())
}
fn render_countdown(countdown: Option<&crate::scanning::Countdown>) -> Result<(), String> {
    COUNTDOWN.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.1.as_ref() == countdown {
            return Ok(());
        }
        if let Some(countdown) = countdown {
            if slot.0.is_none() {
                slot.0 = Some(Host::new()?);
            }
            slot.0.as_mut().unwrap().countdown(countdown)?;
        } else if let Some(host) = slot.0.as_mut() {
            host.hide();
        }
        slot.1 = countdown.cloned();
        Ok(())
    })
}

// Rendering can fail after a native window is shown but before its cache is
// committed. Cleanup must never rely on that cache to decide whether to hide.
fn clear_cached_visual<H, C: Default>(state: &mut (H, C), hide: impl FnOnce(&mut H)) {
    hide(&mut state.0);
    state.1 = C::default();
}

fn hide_scan_visuals() {
    for slot in [&HOST, &PROMPT, &LABEL] {
        slot.with(|host| {
            if let Some(host) = host.borrow_mut().as_mut() {
                host.hide();
            }
        });
    }
    TILES.with(|slot| {
        clear_cached_visual(&mut slot.borrow_mut(), |hosts| {
            for host in hosts {
                host.hide();
            }
        })
    });
    COUNTDOWN.with(|slot| {
        clear_cached_visual(&mut slot.borrow_mut(), |host| {
            if let Some(host) = host {
                host.hide();
            }
        })
    });
}
fn update_tiles(
    previous: &[crate::scanning::FrameTile],
    tiles: &[crate::scanning::FrameTile],
    host_count: usize,
    mut present: impl FnMut(usize, Option<&crate::scanning::FrameTile>) -> Result<(), String>,
) -> Result<(), String> {
    let background_changed = tiles
        .iter()
        .enumerate()
        .any(|(index, tile)| tile.is_panel_background() && previous.get(index) != Some(tile));
    for index in 0..host_count {
        let tile = tiles.get(index);
        if !background_changed && previous.get(index) == tile {
            continue;
        }
        present(index, tile)?;
    }
    Ok(())
}
fn render_tiles(tiles: &[crate::scanning::FrameTile]) -> Result<(), String> {
    TILES.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.1 == tiles {
            return Ok(());
        }
        while slot.0.len() < tiles.len() {
            slot.0.push(Host::new()?);
        }
        let (hosts, previous) = &mut *slot;
        update_tiles(previous, tiles, hosts.len(), |index, tile| {
            if let Some(tile) = tile {
                hosts[index].tile(tile)
            } else {
                hosts[index].hide();
                Ok(())
            }
        })?;
        slot.1 = tiles.to_vec();
        Ok(())
    })
}
fn render<A: Adapter>(
    app: &AppHandle,
    prompt: Option<&crate::switch_gestures::Prompt>,
) -> Result<(), String> {
    let c = app.state::<Controller<A>>();
    let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    let cursor = app.state::<crate::overlay::CursorOverlay>();
    let active = d.engine.as_ref().is_some_and(Session::active) || prompt.is_some();
    if !active {
        hide_scan_visuals();
        if let Some((token, _)) = d.cursor_suppression.take() {
            cursor.release_scan(token);
        }
        return Ok(());
    }
    if active {
        let (token, started) = match d.cursor_suppression {
            Some(lease) => lease,
            None => {
                let lease = (cursor.suppress_for_scan()?, Instant::now());
                d.cursor_suppression = Some(lease);
                lease
            }
        };
        if !cursor.scan_ready(token) {
            if started.elapsed() >= std::time::Duration::from_secs(2) {
                return Err("Cursor overlay could not be hidden for scanning.".into());
            }
            d.last_tick = Instant::now();
            return Ok(());
        }
    }
    let frame = d
        .engine
        .as_ref()
        .map_or_else(Default::default, Session::frame);
    HOST.with(|host| {
        if let Some(host) = host.borrow_mut().as_mut() {
            host.render(&frame.rectangles())
        } else {
            Ok(())
        }
    })?;
    render_tiles(&frame.tiles)?;
    render_countdown(frame.countdown.as_ref())?;
    render_label(frame.label_for_prompt(prompt.is_some()), &frame.tiles)?;
    show_prompt(app, prompt)?;
    Ok(())
}
fn tick<A: Adapter>(app: &AppHandle) {
    if crate::switch_practice::tick(app) {
        return;
    }
    let c = app.state::<Controller<A>>();
    let remote = crate::remote_scan::poll(app);
    let (mut events, local_now, _) = app.state::<switch_runtime::Controller>().poll(app);
    let now_ms = remote.as_ref().map_or(local_now, |r| r.3);
    let was_remote = c.data.lock().unwrap_or_else(|p| p.into_inner()).remote;
    if remote.is_none() && was_remote {
        disable::<A>(app, "Remote scanning stopped. Start forwarding again.");
        return;
    }
    if let Some((generation, settings, edges, _)) = remote {
        let needs_start = {
            let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
            !d.remote || d.input_generation != generation
        };
        if needs_start {
            events.clear();
            reset_scanner::<A>(app, "Starting remote scanning...");
            let start = (|| -> Result<(), String> {
                A::prepare(app)?;
                app.state::<switch_runtime::Controller>().enable_escape()?;
                HOST.with(|slot| {
                    if slot.borrow().is_none() {
                        *slot.borrow_mut() = Some(Host::new()?);
                    }
                    Ok::<_, String>(())
                })?;
                let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
                d.switches = settings.clone();
                d.input_generation = generation;
                d.remote = true;
                d.last_tick = Instant::now();
                d.message = "Remote scanning ready. Press Select on Remote.".into();
                c.enabled.store(true, Ordering::SeqCst);
                Ok(())
            })();
            if let Err(error) = start {
                disable::<A>(app, &error);
                return;
            }
            publish::<A>(app);
        }
        for edge in edges {
            if matches!(edge, crate::remote_scan::Edge::Reset) {
                reset_scanner::<A>(app, "Remote switches changed. Scan reset.");
                return;
            }
            let action = {
                let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
                match edge {
                    crate::remote_scan::Edge::Reset => unreachable!(),
                    crate::remote_scan::Edge::Down(id) => {
                        if !d.pressed.held() {
                            d.remote_hold_started = Some(now_ms);
                        }
                        let countdown = d
                            .engine
                            .as_ref()
                            .is_some_and(|e| e.technique.auto_selecting());
                        d.pressed
                            .pressed_for_scan(&id.to_string(), now_ms, &settings, countdown);
                        None
                    }
                    crate::remote_scan::Edge::Up(id) => {
                        let action = d.pressed.released(&id.to_string(), now_ms);
                        if !d.pressed.held() {
                            d.remote_hold_started = None;
                        }
                        action
                    }
                }
            };
            if let Some(action) = action {
                switch::<A>(app, action, generation, true);
            }
            if !c.enabled.load(Ordering::SeqCst) {
                return;
            }
        }
        let expired = {
            let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
            d.remote_hold_started
                .is_some_and(|start| now_ms.saturating_sub(start) >= settings.escape_ms())
        };
        if expired {
            // Mirror the local emergency hold: reset the scan, keep the
            // session. The next tick restarts remote scanning in place.
            reset_scanner::<A>(app, "Switch held too long. Scan reset.");
            return;
        }
    }
    for event in events {
        if !c.enabled.load(Ordering::SeqCst) {
            break;
        }
        match event {
            Event::Stopped { reason, .. } => {
                crate::remote_scan::cancel(app);
                reset_scanner_for_capture::<A>(
                    app,
                    switch_runtime::stop_message(reason),
                    cfg!(target_os = "windows")
                        && reason == crate::switch_input::StopReason::CaptureLost,
                );
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
                    if d.remote
                        || generation != d.input_generation
                        || !app
                            .state::<switch_runtime::Controller>()
                            .active_generation(generation)
                    {
                        continue;
                    }
                    if action == crate::switch_input::Action::Pressed {
                        let settings = d.switches.clone();
                        let countdown = d
                            .engine
                            .as_ref()
                            .is_some_and(|e| e.technique.auto_selecting());
                        d.pressed
                            .pressed_for_scan(&switch_id, monotonic_ms, &settings, countdown);
                        None
                    } else {
                        d.pressed.released(&switch_id, monotonic_ms)
                    }
                };
                if let Some(action) = selected {
                    switch::<A>(app, action, generation, false);
                }
            }
            _ => {}
        }
    }
    if !c.enabled.load(Ordering::SeqCst) {
        let cleanup = A::cleanup(app);
        crate::remote_scan::record_cleanup(app, cleanup.is_ok());
        ensure::<A>(app);
        return;
    }
    if app.state::<crate::state::AppModel>().snapshot().bluetooth
        == crate::state::BluetoothState::Connected
        && !crate::remote_scan::active(app)
    {
        disable::<A>(app, "Android connected. Local scanning stopped.");
        return;
    }
    let result = (|| -> Result<(), String> {
        let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        let Data {
            display, engine, ..
        } = &mut *d;
        if !A::settle_environment(
            app,
            display.as_mut(),
            engine.as_mut().map(|e| &mut e.technique),
        )? {
            d.last_tick = Instant::now();
            return Ok(());
        }
        A::validate_environment(app, d.display.as_ref())?;
        let now = Instant::now();
        let elapsed = if d.cursor_suppression.is_some_and(|(token, _)| {
            !app.state::<crate::overlay::CursorOverlay>()
                .scan_ready(token)
        }) {
            0
        } else {
            now.duration_since(d.last_tick).as_millis() as u64
        };
        d.last_tick = now;
        let held = d.pressed.held();
        let prompt = d.pressed.prompt(now_ms);
        let mut request = None;
        let captured_keys = prediction_captured_keys(d.remote, &d.switches);
        let phase_changed = if let Some(engine) = d.engine.as_mut() {
            A::poll(app, &mut engine.technique, &captured_keys);
            let before = engine.technique.phase();
            engine.tick(elapsed, held);
            request = engine.take_selection();
            before != engine.technique.phase()
        } else {
            false
        };
        let environment = d.display.clone();
        let input_generation = d.input_generation;
        let remote = d.remote;
        drop(d);
        if let Some(request) = request {
            dispatch::<A>(app, request, environment.as_ref(), input_generation, remote)?;
        }
        if phase_changed {
            publish::<A>(app);
        }
        render::<A>(app, prompt.as_ref())
    })();
    if let Err(error) = result {
        if crate::remote_scan::active(app) && A::ready(app).is_ok() {
            reset_scanner::<A>(app, &error);
        } else {
            disable::<A>(app, &error);
        }
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

fn render_label(
    label: Option<&crate::scanning::FrameLabel>,
    tiles: &[crate::scanning::FrameTile],
) -> Result<(), String> {
    LABEL.with(|slot| {
        let mut host = slot.borrow_mut();
        if let Some(label) = label {
            if host.is_none() {
                *host = Some(Host::new()?);
            }
            host.as_mut()
                .unwrap()
                .label(label, crate::scan_host::menu_title_geometry(label, tiles))?;
        } else if let Some(host) = host.as_mut() {
            host.hide();
        }
        Ok(())
    })
}
fn hide_prompt() {
    let _ = render_countdown(None);
    let _ = render_label(None, &[]);
    hide_hold_prompt();
}
fn hide_hold_prompt() {
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
        hide_hold_prompt();
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
        p.as_mut().unwrap().hud_prompt(
            &format!(
                "Release {} for {}",
                prompt.switch_name,
                prompt.action.label()
            ),
            rect,
            scale,
            crate::scanning::HudPresentation {
                screen: crate::scanning::Rect {
                    x: f64::from(display.x),
                    y: f64::from(display.y),
                    width: f64::from(display.width),
                    height: f64::from(display.height),
                },
                scale,
            },
        )
    })
}

pub fn update_point_setting(
    app: &AppHandle,
    setting: crate::scan_menu::Setting,
) -> Result<(), String> {
    use crate::point_scan_runtime::PointScan;
    let controller = app.state::<Controller<PointScan>>();
    let mut data = controller.data.lock().unwrap_or_else(|p| p.into_inner());
    let mut config = data.config.clone();
    setting.apply(&mut config);
    config.validate()?;
    let path = config_path::<PointScan>(app)?;
    let save = || -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "Cannot save scanning settings.")?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec_pretty(&config).map_err(|_| "Cannot encode scanning settings.")?,
        )
        .map_err(|_| "Cannot save scanning settings.")?;
        std::fs::rename(&temp, &path).map_err(|_| "Cannot save scanning settings.".to_string())
    };
    save()?;
    if let Some(engine) = data.engine.as_mut() {
        engine.technique.apply_config(
            config.point(),
            matches!(
                setting,
                crate::scan_menu::Setting::LineMode | crate::scan_menu::Setting::GridMode
            ),
        );
    }
    data.config = config;
    drop(data);
    publish::<PointScan>(app);
    Ok(())
}
pub fn restart_point_on_display(app: &AppHandle, next: bool) -> Result<(), String> {
    use crate::point_scan_runtime::PointScan;
    let (cursor, displays) = crate::display_navigation::displays(app).map_err(|e| e.message)?;
    let target = crate::display_navigation::cycle_center(cursor, &displays, next)?;
    crate::scan_executor::move_to(target)?;
    let controller = app.state::<Controller<PointScan>>();
    let mut data = controller.data.lock().unwrap_or_else(|p| p.into_inner());
    let (technique, environment) = PointScan::create(app, data.config.clone())?;
    let mut engine = Session::new(technique, data.config.automatic);
    engine.action(Action::Select);
    data.engine = Some(engine);
    data.display = Some(environment);
    data.last_tick = Instant::now();
    Ok(())
}

#[cfg(test)]
mod ownership_tests {
    #[test]
    fn keyboard_updates_preserve_unchanged_tiles_and_restore_background_order() {
        let keyboard = crate::scan_keyboard::Keyboard::new(false);
        let frame = keyboard.frame(
            crate::scanning::Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            1.0,
            crate::scanning::ScannerColor::default(),
        );
        let previous = frame.tiles;
        let mut current = previous.clone();
        current[1].text = "updated".into();
        let mut updates = Vec::new();
        super::update_tiles(&previous, &current, current.len(), |index, tile| {
            updates.push((index, tile.is_some()));
            Ok(())
        })
        .unwrap();
        assert_eq!(updates, vec![(1, true)]);
        updates.clear();
        super::update_tiles(&current, &current, current.len(), |index, _| {
            updates.push((index, true));
            Ok(())
        })
        .unwrap();
        assert!(updates.is_empty());
        current[0].rect.x += 10.0;
        super::update_tiles(&previous, &current, current.len(), |index, _| {
            updates.push((index, true));
            Ok(())
        })
        .unwrap();
        assert_eq!(updates.len(), current.len());
        assert_eq!(updates[0], (0, true));
        updates.clear();
        super::update_tiles(&current, &[], current.len(), |index, tile| {
            updates.push((index, tile.is_some()));
            Ok(())
        })
        .unwrap();
        assert_eq!(updates.len(), current.len());
        assert!(updates.iter().all(|(_, visible)| !visible));
        assert!(
            super::update_tiles(&[], &current, current.len(), |_, _| Err("failed".into())).is_err()
        );
    }

    #[test]
    fn failed_partial_presentations_are_hidden_even_with_empty_caches() {
        let mut tiles = (vec![true, true, false], Vec::<u8>::new());
        super::clear_cached_visual(&mut tiles, |hosts| hosts.fill(false));
        assert!(tiles.0.iter().all(|visible| !visible));
        assert!(tiles.1.is_empty());
        let mut countdown = (Some(true), None::<u8>);
        super::clear_cached_visual(&mut countdown, |host| {
            if let Some(visible) = host {
                *visible = false;
            }
        });
        assert_eq!(countdown, (Some(false), None));
    }

    #[test]
    fn prediction_ignores_only_keys_captured_by_the_current_source() {
        use super::{prediction_captured_keys, Action, Settings};
        let settings = Settings {
            bindings: vec![crate::switches::Binding {
                id: "custom".into(),
                name: "Select".into(),
                key: "F9".into(),
                press_action: Action::Select,
                hold_actions: vec![],
            }],
            ..Settings::default()
        };
        assert_eq!(prediction_captured_keys(false, &settings), ["Escape", "F9"]);
        assert_eq!(prediction_captured_keys(true, &settings), ["Escape"]);
    }
    #[test]
    fn stopped_remote_input_cannot_fall_back_to_a_matching_local_generation() {
        assert!(!super::source_is_current(true, false, true));
        assert!(super::source_is_current(false, false, true));
        assert!(super::source_is_current(true, true, false));
    }
}
