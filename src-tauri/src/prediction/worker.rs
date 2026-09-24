use super::{activity, context, database::Database};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use unicode_segmentation::UnicodeSegmentation;
pub const ARG: &str = "--switchify-prediction-worker";
pub const LIMIT: usize = 16384;

#[derive(Clone, Serialize, Deserialize)]
pub enum Edit {
    Append(String),
    Backspace,
    Reset,
}

// Each edit is scoped before injection begins. Queued edits from before an
// external activity or window change cannot become context for the new target.
#[derive(Clone, Serialize, Deserialize)]
pub struct RecordedEdit {
    pub edit: Edit,
    pub foreground: usize,
    pub time: u64,
}

// Private inherited-pipe payloads. Never log these or forward them to Tauri.
#[derive(Serialize, Deserialize)]
pub enum Request {
    Query {
        generation: u64,
        edits: Vec<RecordedEdit>,
        revision: u64,
        shift: bool,
        caps: bool,
    },
    Accept {
        generation: u64,
        token: u64,
        revision: u64,
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
        revision: u64,
        tracking: bool,
    },
    Insert {
        generation: u64,
        text: Option<String>,
        foreground: Option<usize>,
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
pub struct Engine {
    database: Database,
    foreground: fn() -> Result<usize, ()>,
    observe: fn() -> (u64, bool),
    last_activity: fn() -> u64,
    target: Option<usize>,
    activity: u64,
    tracked: bool,
    buffer: String,
    clipped: bool,
    revision: u64,
    snapshot: Option<String>,
    batch: Option<Batch>,
    suffixes: Vec<String>,
    token: u64,
    case: (bool, bool),
}
impl Engine {
    pub fn new(database: Database, tracked: bool) -> Self {
        Self {
            database,
            tracked,
            foreground: || crate::scan_host::foreground().map_err(|_| ()),
            observe: activity::snapshot,
            last_activity: activity::last_change,
            target: None,
            activity: 0,
            buffer: String::new(),
            clipped: false,
            revision: 0,
            snapshot: None,
            batch: None,
            suffixes: Vec::new(),
            token: 0,
            case: (false, false),
        }
    }
    fn clear(&mut self) {
        self.buffer.clear();
        self.clipped = false;
        self.snapshot = None;
        self.batch = None;
        self.suffixes.clear();
    }
    fn stable(&self, target: usize, epoch: u64) -> bool {
        self.tracked && (self.observe)() == (epoch, true) && (self.foreground)() == Ok(target)
    }
    fn apply_edit(&mut self, edit: Edit) {
        match edit {
            Edit::Reset => self.clear(),
            Edit::Append(text) => {
                if text.chars().count() > 512 {
                    self.clear();
                    return;
                }
                self.buffer.push_str(&text);
                while self.buffer.chars().count() > 512 {
                    let n = self.buffer.graphemes(true).next().unwrap().len();
                    self.buffer.drain(..n);
                    self.clipped = true;
                }
            }
            Edit::Backspace => {
                if let Some((start, _)) = self.buffer.grapheme_indices(true).next_back() {
                    self.buffer.truncate(start);
                } else {
                    self.clear();
                }
            }
        }
    }
    fn query(
        &mut self,
        edits: Vec<RecordedEdit>,
        revision: u64,
        shift: bool,
        caps: bool,
    ) -> Option<Batch> {
        if revision < self.revision || (revision == self.revision && !edits.is_empty()) {
            self.clear();
            return None;
        }
        self.revision = revision;
        let target = match (self.foreground)() {
            Ok(target) => target,
            Err(()) => {
                self.clear();
                self.target = None;
                return None;
            }
        };
        let (epoch, healthy) = (self.observe)();
        if self.target != Some(target) || self.activity != epoch {
            self.clear();
        }
        self.target = Some(target);
        self.activity = epoch;
        if !self.tracked || !healthy || edits.len() > 512 {
            self.clear();
            return None;
        }
        let last_activity = (self.last_activity)();
        for recorded in edits {
            if recorded.foreground == target && recorded.time > last_activity {
                self.apply_edit(recorded.edit);
            } else {
                self.clear();
            }
        }
        if !self.stable(target, epoch) {
            self.clear();
            return None;
        }
        if self.buffer.is_empty() {
            self.batch = None;
            self.snapshot = None;
            return None;
        }
        if self.snapshot.as_ref() == Some(&self.buffer) && self.case == (shift, caps) {
            return self.batch.clone();
        }
        let ctx = context::extract(&self.buffer, self.clipped);
        let words = match self.database.predict(&ctx) {
            Ok(words) => words,
            Err(()) => {
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
        if !self.stable(target, epoch) {
            self.clear();
            return None;
        }
        self.case = (shift, caps);
        self.snapshot = Some(self.buffer.clone());
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
        if !self.stable(self.target?, self.activity) {
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
                edits,
                revision,
                shift,
                caps,
            } => {
                let batch = self.query(edits, revision, shift, caps);
                let tracking = self
                    .target
                    .is_some_and(|target| self.stable(target, self.activity));
                Response::Suggestions {
                    generation,
                    batch,
                    revision: self.revision,
                    tracking,
                }
            }
            Request::Accept {
                generation,
                token,
                revision,
                index,
            } => Response::Insert {
                generation,
                foreground: self.target,
                text: if revision == self.revision {
                    self.accept(token, index)
                } else {
                    self.clear();
                    None
                },
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
        if args.len() != 5 {
            return Err(());
        }
        let path = PathBuf::from(&args[2]);
        let ignored: Vec<u32> = serde_json::from_str(&args[3].to_string_lossy()).map_err(|_| ())?;
        let enhanced: bool = serde_json::from_str(&args[4].to_string_lossy()).map_err(|_| ())?;
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
        activity::set_ignored(ignored);
        let tracked = activity::start();
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let database = Database::open(&path, enhanced)?;
            let mut engine = Engine::new(database, tracked);
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
    fn engine() -> Engine {
        let mut e = Engine::new(Database::fixture(), true);
        e.foreground = || Ok(1);
        e.observe = || (0, true);
        e.last_activity = || 0;
        e
    }
    fn edit(edit: Edit) -> RecordedEdit {
        RecordedEdit {
            edit,
            foreground: 1,
            time: 10,
        }
    }
    fn append(s: &str) -> RecordedEdit {
        edit(Edit::Append(s.into()))
    }
    #[test]
    fn first_letter_and_completion_chain_use_only_buffer() {
        let mut e = engine();
        let first = e.query(vec![append("w")], 1, false, false).unwrap();
        assert!(first.words.iter().any(|w| w == "water"));
        let b = e.query(vec![append("a")], 2, false, false).unwrap();
        assert_eq!(b.words[0], "water");
        let suffix = e.accept(b.token, 0).unwrap();
        assert_eq!(suffix, "ter ");
        assert!(e.accept(b.token, 0).is_none());
        e.query(vec![append(&suffix)], 3, false, false);
        assert_eq!(e.buffer, "water ");
        e.query(vec![edit(Edit::Reset), append("wa")], 4, false, false)
            .unwrap();
        assert_eq!(e.buffer, "wa");
    }
    #[test]
    fn delayed_observer_start_discards_unobserved_edits() {
        let mut e = engine();
        // A Switchify edit at 10 and external activity at 11 both precede
        // observer startup at 12. Only typing after startup is trustworthy.
        e.last_activity = || 12;
        e.query(vec![append("wa")], 1, false, false);
        assert!(e.buffer.is_empty());
        let mut fresh = append("w");
        fresh.time = 13;
        assert!(e.query(vec![fresh], 2, false, false).is_some());
        assert_eq!(e.buffer, "w");
    }
    #[test]
    fn queued_edits_are_scoped_to_window_and_external_activity() {
        let mut e = engine();
        let b = e.query(vec![append("wa")], 1, false, false).unwrap();
        e.observe = || (1, true);
        e.last_activity = || 11;
        assert!(e.accept(b.token, 0).is_none());
        let mut fresh = append("he");
        fresh.time = 12;
        e.query(vec![append("ter"), fresh], 2, false, false);
        assert_eq!(e.buffer, "he");
        e.foreground = || Ok(2);
        e.query(vec![append("wa")], 3, false, false);
        assert!(e.buffer.is_empty());
    }
    #[test]
    fn unicode_backspace_bounds_and_failed_edits_reset_context() {
        let mut e = engine();
        e.query(
            vec![
                append("wae\u{301}👩‍💻"),
                edit(Edit::Backspace),
                edit(Edit::Backspace),
            ],
            1,
            false,
            false,
        );
        assert_eq!(e.buffer, "wa");
        for rev in 2..10 {
            e.query(vec![append(&" word".repeat(90))], rev, false, false);
        }
        assert!(e.buffer.chars().count() <= 512);
        assert!(e.clipped);
        e.query(vec![edit(Edit::Reset)], 10, false, false);
        assert!(e.buffer.is_empty());
        e.query(vec![append("wa"); 513], 11, false, false);
        assert!(e.buffer.is_empty());
    }
    #[test]
    fn stale_revisions_health_and_foreground_races_reject_acceptance() {
        let mut e = engine();
        let b = e.query(vec![append("wa")], 1, false, false).unwrap();
        assert!(matches!(
            e.respond(Request::Accept {
                generation: 0,
                token: b.token,
                revision: 2,
                index: 0
            }),
            Response::Insert { text: None, .. }
        ));
        assert!(e.buffer.is_empty());
        e.query(vec![append("wa")], 2, false, false);
        assert!(e.query(vec![append("wa")], 2, false, false).is_none());
        let b = e.query(vec![append("wa")], 3, false, false).unwrap();
        e.observe = || (0, false);
        assert!(e.accept(b.token, 0).is_none());
        assert!(e.query(vec![append("wa")], 4, false, false).is_none());
    }
    #[test]
    fn casing_and_unchanged_context_keep_choice_identity() {
        let mut e = engine();
        let b = e.query(vec![append("Wa")], 1, false, false).unwrap();
        assert_eq!(b.words[0], "Water");
        assert_eq!(e.query(vec![], 1, false, false).unwrap().token, b.token);
        let upper = e.query(vec![], 1, false, true).unwrap();
        assert_eq!(upper.words[0], "WaTER");
        assert_ne!(upper.token, b.token);
    }
    #[test]
    fn sentence_start_prediction_uses_title_case_without_manual_shift() {
        let mut e = engine();
        let suggestions = e.query(vec![append("Done! ")], 1, false, false).unwrap();
        assert_eq!(suggestions.words[0], "Water");
        let manual_shift = e.query(vec![], 1, true, false).unwrap();
        assert_eq!(manual_shift.words[0], "WATER");
    }
    #[test]
    fn private_frames_are_bounded() {
        assert!(receive::<Request>(&mut &b"bad!"[..]).is_err());
        let request = Request::Query {
            generation: 0,
            revision: 1,
            edits: vec![append(&"x".repeat(LIMIT))],
            shift: false,
            caps: false,
        };
        assert!(send(&mut Vec::new(), &request).is_err());
    }
    #[test]
    #[ignore = "Manual database performance measurement; no desktop input"]
    fn bundled_database_benchmark() {
        let start = std::time::Instant::now();
        let mut db = Database::open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources/word-predictions.lookup"),
            false,
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
