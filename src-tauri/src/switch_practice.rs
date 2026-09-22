//! Bounded switch practice: real gestures and a simple block scanner, no native execution.
use crate::{
    scan_items::{ItemScanner, Policy},
    scan_tree::Node,
    scanning::{Action, Frame, Session, Technique},
    switch_gestures::Gestures,
    switch_input::Event,
    switch_runtime,
    switches::Settings,
};
use serde::Serialize;
use std::{sync::Mutex, time::Instant};
use tauri::{AppHandle, Manager};

const PRACTICE_BLOCKS: [&str; 6] = [
    "Block 1", "Block 2", "Block 3", "Block 4", "Block 5", "Block 6",
];
const PRACTICE_INTERVAL_MS: u64 = 1000;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub active: bool,
    pub source: String,
    pub message: String,
    pub input: String,
    pub action: Option<Action>,
    pub completed: u32,
    pub blocks: Vec<BlockView>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockView {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Block {
    id: usize,
    label: &'static str,
}

struct Blocks {
    scan: ItemScanner<Block>,
    labels: Vec<&'static str>,
}
impl Blocks {
    fn new() -> Self {
        let labels = PRACTICE_BLOCKS.to_vec();
        let nodes = labels
            .iter()
            .enumerate()
            .map(|(id, label)| Node::Leaf(Block { id, label }))
            .collect();
        Self {
            scan: ItemScanner::new(nodes, Policy::MENU),
            labels,
        }
    }
    fn views(&self, scanning: bool) -> Vec<BlockView> {
        let selected = scanning.then(|| self.scan.nav.index());
        self.labels
            .iter()
            .enumerate()
            .map(|(id, label)| BlockView {
                id: id.to_string(),
                label: (*label).into(),
                selected: selected == Some(id),
            })
            .collect()
    }
}
impl Technique for Blocks {
    type Selection = Block;
    type Phase = ();
    fn start(&mut self) {
        self.scan.restart();
    }
    fn advance(&mut self, elapsed_ms: u64) {
        self.scan.advance(elapsed_ms, PRACTICE_INTERVAL_MS);
    }
    fn handle(&mut self, action: Action) -> Option<Self::Selection> {
        self.scan.handle(action)
    }
    fn reset(&mut self) {
        self.scan.restart();
    }
    fn frame(&self) -> Frame {
        Frame::default()
    }
    fn phase(&self) {}
}

struct Practice {
    view: View,
    engine: Session<Blocks>,
    gestures: Gestures,
    settings: Settings,
    generation: u64,
    remote: bool,
    started: Instant,
    last_tick: Instant,
    held_since: Option<u64>,
}
impl Practice {
    fn new(settings: Settings, generation: u64, remote: bool) -> Self {
        let mut practice = Self {
            view: View {
                active: true,
                source: if remote {
                    "Switchify Remote"
                } else {
                    "This computer"
                }
                .into(),
                message: "Use Select to start. Next and Previous move an active scan among the practice blocks.".into(),
                ..View::default()
            },
            engine: Session::new(Blocks::new(), false),
            gestures: Gestures::default(),
            settings,
            generation,
            remote,
            started: Instant::now(),
            last_tick: Instant::now(),
            held_since: None,
        };
        practice.sync_blocks();
        practice
    }
    fn sync_blocks(&mut self) {
        let scanning = self.engine.active();
        self.view.blocks = self.engine.technique.views(scanning);
    }
    fn edge(&mut self, id: &str, pressed: bool, now: u64) {
        let Some(binding) = self.settings.bindings.iter().find(|b| b.id == id) else {
            return;
        };
        self.view.input = binding.name.clone();
        if pressed {
            self.held_since.get_or_insert(now);
            self.gestures.pressed_for_scan(
                id,
                now,
                &self.settings,
                self.engine.technique.auto_selecting(),
            );
            self.view.message = format!(
                "{} received. Release for {} or hold for another action.",
                binding.name,
                binding.press_action.label()
            );
        } else {
            if let Some(action) = self.gestures.released(id, now) {
                self.action(action);
            }
            if !self.gestures.held() {
                self.held_since = None;
            }
        }
    }
    fn action(&mut self, action: Action) {
        self.view.action = Some(action);
        if matches!(action, Action::Stop | Action::Cancel) {
            self.view.active = false;
            return;
        }
        let was_idle = !self.engine.active();
        if let Some(block) = self.engine.action(action) {
            self.complete(&block);
        } else if was_idle && matches!(action, Action::Next | Action::Back) {
            self.view.message =
                "Input received. Select starts a scan; Next/Previous only move an active scan."
                    .into();
        } else if self.engine.active() {
            self.view.message = format!(
                "{} received. Highlight moves among the practice blocks.",
                action.label()
            );
        } else {
            self.view.message = format!(
                "{} received. This practice cannot click or type outside this box.",
                action.label()
            );
        }
        self.sync_blocks();
    }
    fn complete(&mut self, block: &Block) {
        // Intentionally discard every selection — practice never drives native adapters.
        self.view.completed += 1;
        self.view.message = format!(
            "Chose {}. Nothing was sent to another application. Select starts again.",
            block.label
        );
        self.sync_blocks();
    }
    fn tick(&mut self, now: u64) {
        if self.started.elapsed().as_secs() >= 120
            || self
                .held_since
                .is_some_and(|s| now.saturating_sub(s) >= self.settings.escape_ms())
        {
            self.view.active = false;
            return;
        }
        if let Some(prompt) = self.gestures.prompt(now) {
            self.view.message = format!(
                "{}: release for {}",
                prompt.switch_name,
                prompt.action.label()
            );
        }
        self.engine.tick(
            self.last_tick.elapsed().as_millis().min(250) as u64,
            self.gestures.held(),
        );
        self.last_tick = Instant::now();
        self.sync_blocks();
    }
}
#[derive(Default)]
pub struct Controller {
    session: Mutex<Option<Practice>>,
    last: Mutex<View>,
}
pub fn active(app: &AppHandle) -> bool {
    app.try_state::<Controller>().is_some_and(|c| {
        c.session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    })
}
pub fn view(app: &AppHandle) -> View {
    let c = app.state::<Controller>();
    let session = c.session.lock().unwrap_or_else(|p| p.into_inner());
    session.as_ref().map_or_else(
        || c.last.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        |s| s.view.clone(),
    )
}
pub fn stop(app: &AppHandle, message: &str) {
    let Some(c) = app.try_state::<Controller>() else {
        return;
    };
    let mut lock = c.session.lock().unwrap_or_else(|p| p.into_inner());
    let Some(s) = lock.as_mut() else {
        return;
    };
    s.gestures.cancel();
    s.view.active = false;
    s.view.message = message.into();
    drop(lock);
    crate::remote_scan::cancel(app);
    crate::point_scan_runtime::pause(app);
}
pub fn end(app: &AppHandle) {
    stop(app, "Practice finished.");
    if let Some(c) = app.try_state::<Controller>() {
        if let Some(s) = c.session.lock().unwrap_or_else(|p| p.into_inner()).take() {
            *c.last.lock().unwrap_or_else(|p| p.into_inner()) = s.view;
        }
    }
}
pub fn begin(app: &AppHandle, remote: bool) -> Result<View, String> {
    if active(app) {
        if view(app).active {
            return Err("Finish the current practice first.".into());
        }
        end(app);
    }
    let switches = app.state::<switch_runtime::Controller>();
    if switches.view().capture.active {
        return Err("Finish learning before testing switches.".into());
    }
    if remote && !crate::remote_scan::active(app) {
        return Err(
            "No remote forwarding session. Connect Android and start switch forwarding first."
                .into(),
        );
    }
    crate::point_scan_prepare(app)?;
    // Clear stale edges before the practice owns input.
    let remote_state = crate::remote_scan::poll(app);
    let (settings, generation) = if remote {
        let Some((g, s, _, _)) = remote_state else {
            return Err(
                "No remote forwarding session. Connect Android and start switch forwarding first."
                    .into(),
            );
        };
        (s, g)
    } else {
        if remote_state.is_some() {
            return Err("Stop remote forwarding before testing local switches.".into());
        }
        (switches.settings(), 0)
    };
    let mut session = Practice::new(settings, generation, remote);
    crate::point_scan_runtime::pause(app);
    let result = if remote {
        switches.enable_escape()
    } else {
        switches.enable(true)
    };
    result?;
    if !remote {
        session.generation = switches.generation();
    }
    *app.state::<Controller>()
        .session
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(session);
    Ok(view(app))
}
/// Own the whole tick while testing, so no test edge reaches the native executor.
pub fn tick(app: &AppHandle) -> bool {
    if !active(app) {
        return false;
    }
    if !view(app).active {
        return true;
    }
    if !app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
    {
        stop(app, "Practice stopped because Switchify lost focus.");
        return true;
    }
    let remote = crate::remote_scan::poll(app);
    let (events, local_now, _) = app.state::<switch_runtime::Controller>().poll(app);
    let c = app.state::<Controller>();
    let mut lock = c.session.lock().unwrap_or_else(|p| p.into_inner());
    let Some(s) = lock.as_mut() else {
        return true;
    };
    let mut ended = false;
    for event in events {
        match event {
            Event::Stopped { .. } => ended = true,
            Event::Switch {
                generation,
                switch_id,
                action,
                monotonic_ms,
            } if !s.remote && generation == s.generation => s.edge(
                &switch_id,
                action == crate::switch_input::Action::Pressed,
                monotonic_ms,
            ),
            _ => {}
        }
    }
    let now = if s.remote {
        if let Some((generation, _, edges, now)) = remote {
            if generation != s.generation {
                ended = true;
            } else {
                for edge in edges {
                    match edge {
                        crate::remote_scan::Edge::Down(id) => s.edge(&id.to_string(), true, now),
                        crate::remote_scan::Edge::Up(id) => s.edge(&id.to_string(), false, now),
                        crate::remote_scan::Edge::Reset => ended = true,
                    }
                }
            }
            now
        } else {
            ended = true;
            local_now
        }
    } else {
        if remote.is_some() {
            ended = true;
        }
        local_now
    };
    if !ended {
        s.tick(now);
    }
    ended |= !s.view.active;
    drop(lock);
    if ended {
        stop(app, "Practice stopped. Exit or start another test. Remote forwarding must be started again after testing.");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn practice() -> Practice {
        let settings = Settings {
            bindings: vec![crate::switches::Binding {
                id: "one".into(),
                name: "Blue".into(),
                key: "Space".into(),
                press_action: Action::Select,
                hold_actions: vec![Action::Next],
            }],
            ..Default::default()
        };
        Practice::new(settings, 7, false)
    }
    #[test]
    fn input_feedback_and_hold_actions_use_saved_binding() {
        let mut p = practice();
        p.edge("one", true, 0);
        assert_eq!(p.view.input, "Blue");
        p.edge("one", false, 1001);
        assert_eq!(p.view.action, Some(Action::Next));
        assert!(!p.engine.active());
        assert!(p.view.message.contains("Select starts"));
        p.edge("one", true, 1200);
        p.edge("one", false, 1300);
        assert!(p.engine.active());
        assert_eq!(p.view.blocks.iter().filter(|b| b.selected).count(), 1);
    }
    #[test]
    fn selecting_a_block_is_discarded_not_executed() {
        let mut p = practice();
        p.action(Action::Select);
        assert!(p.engine.active());
        p.action(Action::Next);
        p.action(Action::Select);
        assert!(p.view.completed > 0);
        assert!(!p.engine.active());
        assert!(p.view.message.contains("Chose"));
        assert!(p.view.blocks.iter().all(|b| !b.selected));
    }
    #[test]
    fn next_moves_highlight_among_blocks() {
        let mut p = practice();
        p.action(Action::Select);
        let first = p.view.blocks.iter().position(|b| b.selected).unwrap();
        p.action(Action::Next);
        let second = p.view.blocks.iter().position(|b| b.selected).unwrap();
        assert_ne!(first, second);
    }
    #[test]
    fn stop_and_emergency_hold_end_practice() {
        let mut p = practice();
        p.action(Action::Stop);
        assert!(!p.view.active);
        let mut p = practice();
        p.edge("one", true, 0);
        p.tick(p.settings.escape_ms());
        assert!(!p.view.active);
    }
    #[test]
    fn unassigned_input_and_release_without_press_do_nothing() {
        let mut p = practice();
        p.edge("unknown", true, 0);
        p.edge("one", false, 1);
        assert!(p.view.action.is_none());
        assert!(!p.engine.active());
    }
}
