use super::{
    activity,
    context::{self, Adapter, RawContext, Status},
    database::Database,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
pub const ARG: &str = "--switchify-prediction-worker";
pub const LIMIT: usize = 16384;

// Private inherited-pipe payloads. Never log these or forward them to Tauri.
#[derive(Serialize, Deserialize)]
pub enum Request {
    Query {
        generation: u64,
        edit: Option<String>,
        reset: bool,
        shift: bool,
        caps: bool,
    },
    Accept {
        generation: u64,
        token: u64,
        index: usize,
    },
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Batch {
    pub token: u64,
    pub words: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Suggestions {
        generation: u64,
        batch: Option<Batch>,
        tracking: bool,
    },
    Insert {
        generation: u64,
        text: Option<String>,
    },
}
pub fn send<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), ()> {
    let bytes = serde_json::to_vec(value).map_err(|_| ())?;
    if bytes.len() > LIMIT {
        return Err(());
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .map_err(|_| ())?;
    writer.write_all(&bytes).map_err(|_| ())?;
    writer.flush().map_err(|_| ())
}
pub fn receive<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> Result<T, ()> {
    let mut size = [0; 4];
    reader.read_exact(&mut size).map_err(|_| ())?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > LIMIT {
        return Err(());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}
pub struct Engine<A: Adapter> {
    adapter: A,
    database: Database,
    target: Option<A::Target>,
    snapshot: Option<RawContext>,
    fallback: String,
    boundary: bool,
    tracked: bool,
    activity: u64,
    observe: fn() -> (u64, bool),
    batch: Option<Batch>,
    suffixes: Vec<String>,
    token: u64,
    case: (bool, bool),
}
impl<A: Adapter> Engine<A> {
    pub fn new(adapter: A, database: Database, tracked: bool) -> Self {
        Self {
            adapter,
            database,
            tracked,
            target: None,
            snapshot: None,
            fallback: String::new(),
            boundary: false,
            activity: activity::snapshot().0,
            observe: activity::snapshot,
            batch: None,
            suffixes: Vec::new(),
            token: 0,
            case: (false, false),
        }
    }
    fn clear(&mut self) {
        self.fallback.clear();
        self.boundary = false;
        self.snapshot = None;
        self.batch = None;
        self.suffixes.clear();
    }
    fn target(&mut self) -> Result<A::Target, Status> {
        let target = self.adapter.focused()?;
        if self.adapter.protected(&target)? {
            return Err(Status::Protected);
        }
        Ok(target)
    }
    fn stable(&mut self, target: &A::Target, epoch: u64) -> bool {
        self.target()
            .is_ok_and(|current| self.adapter.same(target, &current).unwrap_or(false))
            && (self.observe)().0 == epoch
    }
    fn query(
        &mut self,
        edit: Option<String>,
        reset: bool,
        shift: bool,
        caps: bool,
    ) -> Option<Batch> {
        let target = match self.target() {
            Ok(t) => t,
            Err(_) => {
                self.clear();
                self.target = None;
                return None;
            }
        };
        let (epoch, healthy) = (self.observe)();
        let same = self
            .target
            .as_ref()
            .is_some_and(|old| self.adapter.same(old, &target).unwrap_or(false));
        let uninterrupted = epoch == self.activity;
        if !same || !uninterrupted || reset {
            self.clear();
        }
        if !healthy {
            self.fallback.clear();
            self.boundary = false;
            if self.snapshot.as_ref().is_some_and(|raw| raw.position == -1) {
                self.clear();
            }
        }
        self.activity = epoch;
        if same && uninterrupted && !reset && self.tracked && healthy {
            if let Some(text) = edit {
                if text.chars().count() > 512 {
                    self.clear();
                } else {
                    for c in text.chars() {
                        if c.is_whitespace() || matches!(c, '.' | '!' | '?' | ',' | ';' | ':') {
                            self.boundary = true;
                        }
                        if self.boundary {
                            self.fallback.push(c);
                        }
                    }
                    self.fallback = self
                        .fallback
                        .chars()
                        .rev()
                        .take(512)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                }
            }
        }
        let raw = match self.adapter.read(&target) {
            Ok(raw) if context::eligible(&raw) => raw,
            Err(Status::Unsupported)
                if self.tracked && healthy && self.boundary && self.adapter.editable(&target) =>
            {
                RawContext {
                    before: self.fallback.clone(),
                    after: String::new(),
                    clipped_start: true,
                    has_selection: false,
                    position: -1,
                }
            }
            _ => {
                self.clear();
                self.target = Some(target);
                return None;
            }
        };
        if !self.stable(&target, epoch) {
            self.clear();
            return None;
        }
        self.target = Some(target);
        if self.snapshot.as_ref() == Some(&raw) && self.case == (shift, caps) {
            return self.batch.clone();
        }
        let ctx = context::extract(&raw.before, raw.clipped_start);
        let words = match self.database.predict(&ctx) {
            Ok(w) => w,
            Err(_) => {
                self.clear();
                return None;
            }
        };
        self.token = self.token.wrapping_add(1);
        self.suffixes.clear();
        let mut labels = Vec::new();
        for word in words {
            let offset = word
                .char_indices()
                .nth(ctx.prefix.chars().count())
                .map_or(word.len(), |(i, _)| i);
            if word[..offset].to_lowercase() != ctx.prefix.to_lowercase() {
                continue;
            }
            let mut suffix = word[offset..].to_owned();
            if caps ^ shift {
                suffix = suffix.to_uppercase();
            } else if ctx.prefix.is_empty() && ctx.sentence_start {
                let mut chars = suffix.chars();
                if let Some(c) = chars.next() {
                    suffix = c.to_uppercase().collect::<String>() + chars.as_str();
                }
            }
            labels.push(ctx.prefix.clone() + &suffix);
            self.suffixes.push(suffix + " ");
        }
        self.case = (shift, caps);
        self.snapshot = Some(raw);
        self.batch = Some(Batch {
            token: self.token,
            words: labels,
        });
        self.batch.clone()
    }
    fn accept(&mut self, token: u64, index: usize) -> Option<String> {
        if self.batch.as_ref()?.token != token || index >= self.suffixes.len() {
            return None;
        }
        let target = match self.target() {
            Ok(t) => t,
            Err(_) => {
                self.clear();
                self.target = None;
                return None;
            }
        };
        if !self
            .target
            .as_ref()
            .is_some_and(|old| self.adapter.same(old, &target).unwrap_or(false))
            || self.activity != (self.observe)().0
        {
            self.clear();
            return None;
        }
        let expected = self.snapshot.as_ref()?;
        let valid = if expected.position == -1 {
            self.tracked
                && (self.observe)().1
                && self.boundary
                && self.adapter.editable(&target)
                && matches!(self.adapter.read(&target), Err(Status::Unsupported))
        } else {
            self.adapter
                .read(&target)
                .is_ok_and(|raw| &raw == expected && context::eligible(&raw))
        };
        if !valid || !self.stable(&target, self.activity) {
            self.clear();
            return None;
        }
        let suffix = self.suffixes[index].clone();
        self.batch = None;
        Some(suffix)
    }
    pub fn respond(&mut self, request: Request) -> Response {
        match request {
            Request::Query {
                generation,
                edit,
                reset,
                shift,
                caps,
            } => {
                let mut batch = self.query(edit, reset, shift, caps);
                let current = self.target();
                let stable = current.as_ref().is_ok_and(|current| {
                    self.target
                        .as_ref()
                        .is_some_and(|old| self.adapter.same(old, current).unwrap_or(false))
                });
                if !stable {
                    self.clear();
                    batch = None;
                }
                let tracking = stable
                    && self.tracked
                    && (self.observe)().1
                    && current.is_ok_and(|target| self.adapter.editable(&target));
                Response::Suggestions {
                    generation,
                    batch,
                    tracking,
                }
            }
            Request::Accept {
                generation,
                token,
                index,
            } => Response::Insert {
                generation,
                text: self.accept(token, index),
            },
        }
    }
}
pub fn run_from_args() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|s| s != ARG) {
        return false;
    }
    let work = || -> Result<(), ()> {
        if args.len() != 4 {
            return Err(());
        }
        let path = PathBuf::from(&args[2]);
        let ignored: Vec<u32> = serde_json::from_str(&args[3].to_string_lossy()).map_err(|_| ())?;
        if ignored.len() > 129 {
            return Err(());
        }
        #[cfg(target_os = "macos")]
        {
            let parent = unsafe { libc::getppid() };
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if unsafe { libc::getppid() } != parent {
                    std::process::exit(0);
                }
            });
        }
        let tracked = activity::start(ignored);
        #[cfg(target_os = "windows")]
        let adapter = super::windows::WindowsAdapter::new().map_err(|_| ())?;
        #[cfg(target_os = "macos")]
        let adapter = super::macos::MacAdapter::new().map_err(|_| ())?;
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let database = Database::open(&path)?;
            let mut engine = Engine::new(adapter, database, tracked);
            let mut input = std::io::stdin().lock();
            let mut output = std::io::stdout().lock();
            while let Ok(request) = receive::<Request>(&mut input) {
                send(&mut output, &engine.respond(request))?;
            }
        }
        Ok(())
    };
    let _ = work();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        raw: RawContext,
        target: usize,
        protected: bool,
        unsupported: bool,
        editable: bool,
        reads: usize,
        change_on_read: bool,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                raw: RawContext {
                    before: "I like wa".into(),
                    after: String::new(),
                    clipped_start: false,
                    has_selection: false,
                    position: 9,
                },
                target: 1,
                protected: false,
                unsupported: false,
                editable: true,
                reads: 0,
                change_on_read: false,
            }
        }
    }
    impl Adapter for Fake {
        type Target = usize;
        fn focused(&mut self) -> Result<usize, Status> {
            Ok(self.target)
        }
        fn protected(&mut self, _: &usize) -> Result<bool, Status> {
            Ok(self.protected)
        }
        fn editable(&mut self, _: &usize) -> bool {
            self.editable
        }
        fn same(&mut self, a: &usize, b: &usize) -> Result<bool, Status> {
            Ok(a == b)
        }
        fn read(&mut self, _: &usize) -> Result<RawContext, Status> {
            self.reads += 1;
            if self.change_on_read {
                self.target += 1;
            }
            if self.unsupported {
                Err(Status::Unsupported)
            } else {
                Ok(self.raw.clone())
            }
        }
    }
    fn engine() -> Engine<Fake> {
        let mut e = Engine::new(Fake::new(), Database::fixture(), true);
        e.observe = || (0, true);
        e.activity = 0;
        e
    }
    #[test]
    fn native_predictions_survive_polls_without_fallback_tracking() {
        for unavailable in 0..3 {
            let mut e = engine();
            match unavailable {
                0 => e.tracked = false,
                1 => e.adapter.editable = false,
                _ => e.observe = || (0, false),
            }
            let mut service = crate::prediction::Service::default();
            let mut keyboard = crate::scan_keyboard::Keyboard::new(false);
            keyboard.enable_predictions(true);
            let mut original = None;
            for _ in 0..40 {
                let response = e.respond(Request::Query {
                    generation: 0,
                    edit: service.edit.take(),
                    reset: std::mem::take(&mut service.reset),
                    shift: false,
                    caps: false,
                });
                let Response::Suggestions {
                    batch, tracking, ..
                } = response
                else {
                    panic!("expected suggestions");
                };
                assert!(!tracking);
                let batch = batch.unwrap();
                let expected = original.get_or_insert_with(|| batch.clone());
                assert_eq!(batch.token, expected.token);
                assert_eq!(batch.words, expected.words);
                service.suggestions(&mut keyboard, Some(batch), tracking);
                keyboard.advance(250, 1000);
            }
            let token = original.unwrap().token;
            assert_eq!(e.accept(token, 0), Some("ter ".into()));
            assert!(e.accept(token, 0).is_none());
        }
    }
    #[test]
    fn genuine_context_changes_invalidate_identical_native_candidates() {
        for change in 0..4 {
            let mut e = engine();
            e.adapter.editable = false;
            let original = e.query(None, false, false, false).unwrap();
            match change {
                0 => e.adapter.target += 1,
                1 => e.adapter.raw.position += 1,
                2 => e.activity = u64::MAX,
                _ => {}
            }
            let replacement = e.query(None, change == 3, false, false).unwrap();
            assert_eq!(replacement.words, original.words);
            assert_ne!(replacement.token, original.token);
            assert!(e.accept(original.token, 0).is_none());
            assert_eq!(e.accept(replacement.token, 0), Some("ter ".into()));
        }
    }
    #[test]
    fn native_edits_selection_and_casing_refresh_without_tracking() {
        let mut e = engine();
        e.tracked = false;
        let original = e.query(None, false, false, false).unwrap();
        e.adapter.raw.before.push('t');
        let typed = e.query(None, false, false, false).unwrap();
        assert_ne!(typed.token, original.token);
        assert!(e.accept(original.token, 0).is_none());
        e.adapter.raw.before.pop();
        let deleted = e.query(None, false, false, false).unwrap();
        assert_ne!(deleted.token, typed.token);
        let shifted = e.query(None, false, true, false).unwrap();
        assert_ne!(shifted.token, deleted.token);
        assert!(e.accept(deleted.token, 0).is_none());
        e.adapter.raw.has_selection = true;
        assert!(e.query(None, false, false, false).is_none());
        assert!(e.accept(shifted.token, 0).is_none());
        e.adapter.raw.has_selection = false;
        let restored = e.query(None, false, false, false).unwrap();
        e.adapter.protected = true;
        assert!(e.query(None, false, false, false).is_none());
        assert!(e.accept(restored.token, 0).is_none());
    }
    #[test]
    fn losing_tracking_does_not_erase_a_pending_context_reset() {
        let mut service = crate::prediction::Service {
            reset: true,
            edit: Some("queued".into()),
            ..Default::default()
        };
        service.suggestions(&mut crate::scan_keyboard::Keyboard::new(false), None, false);
        assert!(service.reset);
        assert!(service.edit.is_none());
    }
    #[test]
    fn only_suffix_and_space_are_returned_once() {
        let mut e = engine();
        let b = e.query(None, false, false, false).unwrap();
        assert_eq!(b.words[0], "water");
        assert_eq!(e.accept(b.token, 0), Some("ter ".into()));
        assert!(e.accept(b.token, 0).is_none());
    }
    #[test]
    fn protected_fields_are_not_read_and_focus_races_discard_results() {
        let mut e = engine();
        e.adapter.protected = true;
        assert!(e.query(None, false, false, false).is_none());
        assert_eq!(e.adapter.reads, 0);
        e.adapter.protected = false;
        e.adapter.change_on_read = true;
        assert!(e.query(None, false, false, false).is_none());
    }
    #[test]
    fn selection_middle_word_and_stale_acceptance_are_rejected() {
        let mut e = engine();
        e.adapter.raw.has_selection = true;
        assert!(e.query(None, false, false, false).is_none());
        e.adapter.raw.has_selection = false;
        e.adapter.raw.after = "lk".into();
        assert!(e.query(None, false, false, false).is_none());
        e.adapter.raw.after.clear();
        let b = e.query(None, false, false, false).unwrap();
        e.adapter.raw.position += 1;
        assert!(e.accept(b.token, 0).is_none());
    }
    #[test]
    fn fallback_requires_boundary_and_resets_on_edits_and_focus() {
        let mut e = engine();
        e.adapter.unsupported = true;
        assert!(e.query(None, false, false, false).is_none());
        assert!(e.query(Some("wa".into()), false, false, false).is_none());
        let b = e.query(Some(" wa".into()), false, false, false).unwrap();
        assert_eq!(e.accept(b.token, 0), Some("ter ".into()));
        assert!(e.query(None, true, false, false).is_none());
        e.query(Some(" wa".into()), false, false, false).unwrap();
        e.adapter.target = 2;
        assert!(e.query(None, false, false, false).is_none());
    }
    #[test]
    fn external_activity_discards_edits_queued_before_the_change() {
        let mut e = engine();
        e.adapter.unsupported = true;
        assert!(e.query(None, false, false, false).is_none());
        e.query(Some(" wa".into()), false, false, false).unwrap();
        e.activity = 0u64.wrapping_sub(1);
        assert!(e.query(Some(" wa".into()), false, false, false).is_none());
        assert!(e.fallback.is_empty());
        assert!(!e.boundary);
    }
    #[test]
    fn lost_observer_disables_fallback_query_and_acceptance() {
        let mut e = engine();
        e.adapter.unsupported = true;
        e.query(None, false, false, false);
        let batch = e.query(Some(" wa".into()), false, false, false).unwrap();
        e.observe = || (0, false);
        assert!(e.accept(batch.token, 0).is_none());
        assert!(e.query(Some(" wa".into()), false, false, false).is_none());
        assert!(!e.boundary);
        assert!(matches!(
            e.respond(Request::Query {
                generation: 1,
                edit: None,
                reset: false,
                shift: false,
                caps: false,
            }),
            Response::Suggestions {
                tracking: false,
                batch: None,
                ..
            }
        ));
    }
    #[test]
    fn casing_and_unchanged_snapshots_keep_choice_identity() {
        let mut e = engine();
        e.adapter.raw.before = "Wa".into();
        let b = e.query(None, false, false, false).unwrap();
        assert_eq!(b.words[0], "Water");
        assert_eq!(e.query(None, false, false, false).unwrap().token, b.token);
        let upper = e.query(None, false, false, true).unwrap();
        assert_ne!(upper.token, b.token);
        assert_eq!(upper.words[0], "WaTER");
        assert!(e.accept(b.token, 0).is_none());
    }
    #[test]
    fn malformed_and_oversized_private_frames_fail_closed() {
        assert!(receive::<Request>(&mut &b"bad!"[..]).is_err());
        let request = Request::Query {
            generation: 1,
            edit: Some("x".repeat(LIMIT)),
            reset: false,
            shift: false,
            caps: false,
        };
        assert!(send(&mut Vec::new(), &request).is_err());
    }
    #[test]
    #[ignore = "Manual database performance measurement; no desktop input"]
    fn bundled_database_benchmark() {
        let start = std::time::Instant::now();
        let db = Database::open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources/WordData2017051601.db"),
        )
        .unwrap();
        let startup = start.elapsed().as_secs_f64() * 1000.0;
        let mut times = Vec::new();
        for i in 0..200 {
            let text = [
                "I would like wa",
                "the",
                "hello ",
                "I would like some ",
                "zzzx",
            ][i % 5];
            let ctx = context::extract(text, false);
            let t = std::time::Instant::now();
            assert!(db.predict(&ctx).is_ok());
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!("database startup_ms={startup:.2} samples=200 failures=0 median_ms={:.2} p95_ms={:.2} max_ms={:.2}",times[99],times[189],times[199]);
    }
}
