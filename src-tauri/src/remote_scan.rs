//! Remote switch sessions carry edges, never keyboard input. Transport validation
//! and authentication happen before this bounded, generation-scoped mailbox.
use crate::{
    scanning::Action,
    switches::{Binding, Settings},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashSet, VecDeque},
    sync::Mutex,
    time::Instant,
};
use tauri::{AppHandle, Manager};
pub const PROFILE_ID: &str = "builtin.switchify-scanning";
const MAX_EVENTS: usize = 64;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slot {
    pub press_action: Option<Action>,
    pub hold_actions: Vec<Action>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub revision: u32,
    pub slots: Vec<Slot>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 1,
            slots: [
                Some(Action::Select),
                Some(Action::Next),
                Some(Action::Back),
                Some(Action::Pause),
                Some(Action::Reverse),
                Some(Action::Stop),
                None,
                None,
            ]
            .into_iter()
            .map(|press_action| Slot {
                press_action,
                hold_actions: vec![],
            })
            .collect(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.revision == 0
            || self.slots.len() != 8
            || self.slots.iter().any(|s| {
                s.hold_actions.len() > 32
                    || s.press_action == Some(Action::Cancel)
                    || s.hold_actions.contains(&Action::Cancel)
                    || (s.press_action.is_none() && !s.hold_actions.is_empty())
            })
        {
            return Err("Choose valid actions for eight remote switch slots.".into());
        }
        Ok(())
    }
    pub fn settings(&self, count: usize, interval: u64) -> Settings {
        Settings {
            schema_version: 1,
            hold_interval_ms: interval,
            bindings: self
                .slots
                .iter()
                .take(count)
                .enumerate()
                .filter_map(|(i, s)| {
                    Some(Binding {
                        id: (i + 1).to_string(),
                        name: format!("Remote switch {}", i + 1),
                        key: format!("F{}", i + 1),
                        press_action: s.press_action?,
                        hold_actions: s.hold_actions.clone(),
                    })
                })
                .collect(),
        }
    }
    pub fn profile(&self) -> Value {
        json!({"id":PROFILE_ID,"name":"Switchify scanning","version":self.revision,"kind":"scanning","bindings":self.slots.iter().enumerate().map(|(i,s)| {
        let mut label = s.press_action.map_or("Unassigned", Action::label).to_string();
        if !s.hold_actions.is_empty() { label.push_str("; hold: "); label.push_str(&s.hold_actions.iter().map(|a| a.label()).collect::<Vec<_>>().join(", ")); }
        json!({"switchId":i+1,"label":label,"behavior":if s.press_action.is_some(){"stateful"}else{"unassigned"}})
    }).collect::<Vec<_>>()})
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Edge {
    Down(u8),
    Up(u8),
    Reset,
}
struct Session {
    device: String,
    id: String,
    sequence: i64,
    count: u8,
    pressed: HashSet<u8>,
    neutral: bool,
    last_seen: u64,
    settings: Settings,
}
pub struct Mailbox {
    pub config: Config,
    session: Option<Session>,
    generation: u64,
    queue: VecDeque<Edge>,
}
impl Mailbox {
    fn new(config: Config) -> Self {
        Self {
            config,
            session: None,
            generation: 0,
            queue: VecDeque::new(),
        }
    }
    fn stop(&mut self) {
        self.session = None;
        self.queue.clear();
        self.generation = self.generation.wrapping_add(1);
    }
    fn expire(&mut self, now: u64) {
        if self
            .session
            .as_ref()
            .is_some_and(|s| now.saturating_sub(s.last_seen) >= 5000)
        {
            self.stop();
        }
    }
    fn start(
        &mut self,
        device: &str,
        payload: &Value,
        interval: u64,
        automatic: bool,
        now: u64,
    ) -> Result<(), String> {
        let count = payload["switchCount"]
            .as_u64()
            .filter(|n| (1..=8).contains(n))
            .ok_or("Invalid remote switch count")? as u8;
        if payload["profileVersion"].as_u64() != Some(self.config.revision.into()) {
            return Err("The scanning profile changed. Reload profiles and start again.".into());
        }
        let id = payload["sessionId"]
            .as_str()
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
            .ok_or("Invalid session ID")?;
        let settings = self.config.settings(count as usize, interval);
        settings.validate_actions(automatic)?;
        self.stop();
        self.session = Some(Session {
            device: device.into(),
            id: id.into(),
            count,
            sequence: 0,
            pressed: HashSet::new(),
            neutral: true,
            last_seen: now,
            settings,
        });
        Ok(())
    }
    fn accept(
        &mut self,
        device: &str,
        command: &str,
        payload: &Value,
        now: u64,
    ) -> Result<(), String> {
        let session = self
            .session
            .as_mut()
            .ok_or("Remote scanning stopped. Start again.")?;
        if session.device != device || payload["sessionId"].as_str() != Some(&session.id) {
            return Err("Remote scanning session does not match.".into());
        }
        let sequence = payload["sequence"]
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or("Invalid switch sequence")?;
        if sequence <= session.sequence {
            return Ok(());
        }
        if command == "switch.session.stop" {
            self.stop();
            return Ok(());
        }
        if command == "switch.sync" {
            let pressed: HashSet<u8> = payload["pressedSwitchIds"]
                .as_array()
                .ok_or("Invalid held switches")?
                .iter()
                .map(|v| {
                    v.as_u64()
                        .filter(|n| *n > 0 && *n <= session.count as u64)
                        .map(|n| n as u8)
                        .ok_or("Invalid switch slot")
                })
                .collect::<Result<_, _>>()?;
            if pressed != session.pressed {
                self.queue.clear();
                self.queue.push_back(Edge::Reset);
                session.neutral = true;
            }
            session.pressed = pressed;
            if session.pressed.is_empty() {
                session.neutral = false;
            }
        } else {
            let id = payload["switchId"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= session.count as u64)
                .ok_or("Invalid switch slot")? as u8;
            let down = match payload["state"].as_str() {
                Some("down") => true,
                Some("up") => false,
                _ => return Err("Invalid switch state".into()),
            };
            if sequence != session.sequence + 1 {
                self.queue.clear();
                self.queue.push_back(Edge::Reset);
                session.neutral = true;
            }
            let changed = if down {
                session.pressed.insert(id)
            } else {
                session.pressed.remove(&id)
            };
            if !session.neutral && changed {
                self.queue
                    .push_back(if down { Edge::Down(id) } else { Edge::Up(id) });
            }
            if session.neutral && session.pressed.is_empty() {
                session.neutral = false;
            }
        }
        session.sequence = sequence;
        session.last_seen = now;
        if self.queue.len() > MAX_EVENTS {
            self.stop();
            return Err("Remote switch queue overflow. Start again.".into());
        }
        Ok(())
    }
}
pub struct Controller {
    data: Mutex<Mailbox>,
    clock: Instant,
}
fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("remote-switch-settings.json"))
}
pub fn install(app: &AppHandle) {
    let config = path(app)
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
        .filter(|c| c.validate().is_ok())
        .unwrap_or_default();
    app.manage(Controller {
        data: Mutex::new(Mailbox::new(config)),
        clock: Instant::now(),
    });
}
pub fn config(app: &AppHandle) -> Config {
    app.state::<Controller>()
        .data
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .config
        .clone()
}
pub fn save(app: &AppHandle, mut config: Config) -> Result<Config, String> {
    config.validate()?;
    let controller = app.state::<Controller>();
    let mut data = controller.data.lock().unwrap_or_else(|p| p.into_inner());
    config.revision = data
        .config
        .revision
        .checked_add(1)
        .ok_or("Scanning profile revision exhausted")?;
    let p = path(app)?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        p,
        serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    data.stop();
    data.config = config.clone();
    Ok(config)
}
pub fn cancel(app: &AppHandle) {
    if let Some(c) = app.try_state::<Controller>() {
        c.data.lock().unwrap_or_else(|p| p.into_inner()).stop();
    }
}
pub fn active(app: &AppHandle) -> bool {
    app.try_state::<Controller>().is_some_and(|c| {
        c.data
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .session
            .is_some()
    })
}
pub fn allow_direct(app: &AppHandle) -> Result<(), String> {
    if active(app) {
        Err("Stop Switchify scanning before using other PC controls.".into())
    } else {
        Ok(())
    }
}
pub fn active_generation(app: &AppHandle, generation: u64) -> bool {
    app.try_state::<Controller>().is_some_and(|c| {
        let d = c.data.lock().unwrap_or_else(|p| p.into_inner());
        d.session.is_some() && d.generation == generation
    })
}
pub fn poll(app: &AppHandle) -> Option<(u64, Settings, Vec<Edge>, u64)> {
    let c = app.state::<Controller>();
    let now = c.clock.elapsed().as_millis() as u64;
    let mut d = c.data.lock().unwrap_or_else(|p| p.into_inner());
    d.expire(now);
    let settings = d.session.as_ref()?.settings.clone();
    let generation = d.generation;
    let edges = d.queue.drain(..).collect();
    Some((generation, settings, edges, now))
}
/// Called only for authenticated commands. Native input is released by the caller
/// before a start is routed here; actual scanning runs on the next main-thread tick.
pub fn route(
    app: &AppHandle,
    device: &str,
    command: &str,
    payload: &Value,
) -> Option<Result<(), String>> {
    let scanning_start = command == "switch.session.start" && payload["profileId"] == PROFILE_ID;
    if command == "switch.session.start" {
        crate::point_scan_runtime::pause(app);
        if let Err(error) = crate::scan_executor::cleanup() {
            return Some(Err(error));
        }
    }
    if scanning_start {
        let view = app.state::<crate::point_scan_runtime::Controller>().view();
        let interval = app
            .state::<crate::switch_runtime::Controller>()
            .settings()
            .hold_interval_ms;
        let c = app.state::<Controller>();
        let now = c.clock.elapsed().as_millis() as u64;
        if app
            .state::<crate::state::AppModel>()
            .snapshot()
            .accessibility
            != crate::state::AccessibilityState::Granted
        {
            return Some(Err("Grant input access before remote scanning.".into()));
        }
        return Some(c.data.lock().unwrap_or_else(|p| p.into_inner()).start(
            device,
            payload,
            interval,
            view.config.automatic,
            now,
        ));
    }
    if command == "switch.session.start" || command == "connection.disconnecting" {
        cancel(app);
        return None;
    }
    if command == "switch.profile.list" || !active(app) {
        return None;
    }
    if matches!(
        command,
        "switch.edge" | "switch.sync" | "switch.session.stop"
    ) {
        let c = app.state::<Controller>();
        let now = c.clock.elapsed().as_millis() as u64;
        return Some(
            c.data
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .accept(device, command, payload, now),
        );
    }
    Some(Err(
        "Stop Switchify scanning before using other PC controls.".into(),
    ))
}
pub fn catalog(app: &AppHandle, payload: &Value, response: String) -> String {
    if payload["includeScanning"] != true {
        return response;
    }
    let mut value: Value = serde_json::from_str(&response).expect("local catalog JSON");
    value["payload"]["profiles"]
        .as_array_mut()
        .expect("local catalog profiles")
        .push(config(app).profile());
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    fn start(count: u8) -> Value {
        json!({"sessionId":ID,"profileVersion":1,"switchCount":count})
    }
    fn edge(seq: i64, id: u8, state: &str) -> Value {
        json!({"sessionId":ID,"sequence":seq,"switchId":id,"state":state})
    }
    fn sync(seq: i64, held: Vec<u8>) -> Value {
        json!({"sessionId":ID,"sequence":seq,"pressedSwitchIds":held})
    }
    fn ready() -> Mailbox {
        let mut m = Mailbox::new(Config::default());
        m.start("peer", &start(8), 1000, true, 0).unwrap();
        m.accept("peer", "switch.sync", &sync(1, vec![]), 0)
            .unwrap();
        m
    }
    #[test]
    fn timeout_invalidates_polled_work_and_never_restores_a_session() {
        let mut m = ready();
        m.accept("peer", "switch.edge", &edge(2, 1, "down"), 10)
            .unwrap();
        let polled_generation = m.generation;
        let polled: Vec<_> = m.queue.drain(..).collect();
        assert_eq!(polled, vec![Edge::Down(1)]);
        m.expire(5009);
        assert!(m.session.is_some());
        m.expire(5010);
        assert!(m.session.is_none());
        assert_ne!(m.generation, polled_generation);
        assert!(m
            .accept("peer", "switch.sync", &sync(3, vec![]), 5011)
            .is_err());
    }
    #[test]
    fn defaults_validate_connected_slots_and_revision() {
        let mut m = Mailbox::new(Config::default());
        m.start("peer", &start(1), 1000, true, 0).unwrap();
        assert!(m.start("peer", &start(1), 1000, false, 0).is_err());
        m.start("peer", &start(3), 1000, false, 0).unwrap();
        let mut changed = start(3);
        changed["profileVersion"] = json!(2);
        assert!(m.start("peer", &changed, 1000, true, 0).is_err());
        serde_json::from_slice::<Config>(&serde_json::to_vec(&m.config).unwrap())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(m.config.profile()["kind"], "scanning");
    }
    #[test]
    fn duplicates_wrong_owner_and_invalid_slots_do_not_select() {
        let mut m = ready();
        assert!(m
            .accept("other", "switch.edge", &edge(2, 1, "down"), 1)
            .is_err());
        assert!(m
            .accept("peer", "switch.edge", &edge(2, 9, "down"), 1)
            .is_err());
        for (seq, state) in [(2, "down"), (2, "down"), (3, "up"), (3, "up")] {
            m.accept("peer", "switch.edge", &edge(seq, 1, state), seq as u64)
                .unwrap();
        }
        assert_eq!(m.queue, VecDeque::from([Edge::Down(1), Edge::Up(1)]));
    }
    #[test]
    fn missing_edges_and_resync_cancel_instead_of_selecting() {
        let mut m = ready();
        m.accept("peer", "switch.edge", &edge(2, 1, "down"), 1)
            .unwrap();
        m.accept("peer", "switch.sync", &sync(3, vec![]), 2)
            .unwrap();
        assert_eq!(m.queue, VecDeque::from([Edge::Reset]));
        m.queue.clear();
        m.accept("peer", "switch.edge", &edge(5, 1, "up"), 3)
            .unwrap();
        assert_eq!(m.queue, VecDeque::from([Edge::Reset]));
        m.accept("peer", "switch.edge", &edge(6, 1, "down"), 4)
            .unwrap();
        m.accept("peer", "switch.edge", &edge(7, 1, "up"), 5)
            .unwrap();
        assert_eq!(
            m.queue,
            VecDeque::from([Edge::Reset, Edge::Down(1), Edge::Up(1)])
        );
    }
    #[test]
    fn stop_and_overflow_discard_queued_input() {
        let mut m = ready();
        let generation = m.generation;
        m.accept("peer", "switch.edge", &edge(2, 1, "down"), 1)
            .unwrap();
        m.accept(
            "peer",
            "switch.session.stop",
            &json!({"sessionId":ID,"sequence":3}),
            2,
        )
        .unwrap();
        assert!(m.queue.is_empty());
        assert!(m.session.is_none());
        assert_ne!(generation, m.generation);
        let mut m = ready();
        for seq in 2..=66 {
            let _ = m.accept(
                "peer",
                "switch.edge",
                &edge(seq, 1, if seq % 2 == 0 { "down" } else { "up" }),
                seq as u64,
            );
        }
        assert!(m.session.is_none());
        assert!(m.queue.is_empty());
    }
    #[test]
    fn remote_edges_use_shared_release_and_hold_gestures() {
        let mut m = ready();
        m.config.slots[0].hold_actions = vec![Action::Next];
        let settings = m.config.settings(8, 1000);
        let mut g = crate::switch_gestures::Gestures::default();
        m.accept("peer", "switch.edge", &edge(2, 1, "down"), 10)
            .unwrap();
        for e in m.queue.drain(..) {
            if let Edge::Down(id) = e {
                g.pressed(&id.to_string(), 10, &settings);
            }
        }
        assert!(g.held());
        assert_eq!(g.prompt(1010).unwrap().action, Action::Next);
        m.accept("peer", "switch.edge", &edge(3, 1, "up"), 1010)
            .unwrap();
        for e in m.queue.drain(..) {
            if let Edge::Up(id) = e {
                assert_eq!(g.released(&id.to_string(), 1010), Some(Action::Next));
            }
        }
        g.pressed("1", 2000, &settings);
        g.cancel();
        assert_eq!(g.released("1", 2200), None);
    }
}
