use super::{
    activity, context,
    database::{Database, Status},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use unicode_segmentation::UnicodeSegmentation;
pub const ARG: &str = "--switchify-prediction-worker";
pub const LIMIT: usize = 16384;

/// The keyboard's Shift as it applies to a suggestion. Once acts like it does
/// on a typed letter: it changes the first character only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shift {
    Off,
    Once,
    Locked,
}

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
        shift: Shift,
        caps: bool,
        /// The keyboard's pending automatic capital, the one place that decides
        /// a sentence start: it saw the punctuation, and whether the person
        /// cancelled the capital since.
        sentence_start: bool,
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
        status: Status,
    },
    Insert {
        generation: u64,
        /// Characters to delete before typing `text`: the typed prefix, when
        /// accepting restores a capital in it. Never more than the prefix.
        backspaces: usize,
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
    status: Status,
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
    /// Per suggestion: how many typed characters to delete, then the text.
    inserts: Vec<(usize, String)>,
    token: u64,
    case: (Shift, bool, bool),
}
impl Engine {
    pub fn new(database: Database, tracked: bool) -> Self {
        Self {
            database,
            status: Status::Loading,
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
            inserts: Vec::new(),
            token: 0,
            case: (Shift::Off, false, false),
        }
    }
    fn clear(&mut self) {
        self.buffer.clear();
        self.clipped = false;
        self.snapshot = None;
        self.batch = None;
        self.inserts.clear();
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
        shift: Shift,
        caps: bool,
        sentence_start: bool,
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
        self.status = self.database.status();
        if self.snapshot.as_ref() == Some(&self.buffer)
            && self.case == (shift, caps, sentence_start)
            && self.status != Status::Loading
        {
            return self.batch.clone();
        }
        let ctx = context::extract(&self.buffer, self.clipped);
        let (status, prediction) = self.database.predict(&ctx);
        let words = prediction.words;
        self.status = status;
        self.token = self.token.wrapping_add(1);
        self.inserts.clear();
        let mut labels = Vec::new();
        for word in words {
            let Some(offset) = prefix_offset(&word, &ctx.prefix) else {
                continue;
            };
            let suffix = if shouting(&ctx.prefix) {
                word[offset..].to_uppercase()
            } else {
                cased(
                    &word[offset..],
                    ctx.prefix.is_empty(),
                    sentence_start,
                    shift,
                    caps,
                )
            };
            // A word written in capitals keeps its typed prefix as it is.
            let head = if shouting(&ctx.prefix) {
                None
            } else {
                restored(&ctx.prefix, &word[..offset])
            };
            match head {
                Some(head) => {
                    labels.push(format!("{head}{suffix}"));
                    self.inserts.push((
                        ctx.prefix.graphemes(true).count(),
                        format!("{head}{suffix} "),
                    ));
                }
                None => {
                    labels.push(ctx.prefix.clone() + &suffix);
                    self.inserts.push((0, suffix + " "));
                }
            }
        }
        if !self.stable(target, epoch) {
            self.clear();
            return None;
        }
        self.case = (shift, caps, sentence_start);
        self.snapshot = Some(self.buffer.clone());
        self.batch = Some(Batch {
            token: self.token,
            words: labels,
        });
        self.batch.clone()
    }
    fn accept(&mut self, token: u64, index: usize) -> Option<(usize, String)> {
        if self.batch.as_ref()?.token != token || index >= self.inserts.len() {
            return None;
        }
        if !self.stable(self.target?, self.activity) {
            self.clear();
            return None;
        }
        let insert = self.inserts[index].clone();
        self.batch = None;
        Some(insert)
    }
    pub fn respond(&mut self, request: Request) -> Response {
        match request {
            Request::Query {
                generation,
                edits,
                revision,
                shift,
                caps,
                sentence_start,
            } => {
                let batch = self.query(edits, revision, shift, caps, sentence_start);
                self.status = self.database.status();
                let tracking = self
                    .target
                    .is_some_and(|target| self.stable(target, self.activity));
                Response::Suggestions {
                    generation,
                    batch,
                    revision: self.revision,
                    tracking,
                    status: self.status,
                }
            }
            Request::Accept {
                generation,
                token,
                revision,
                index,
            } => {
                let insert = if revision == self.revision {
                    self.accept(token, index)
                } else {
                    self.clear();
                    None
                };
                let (backspaces, text) = insert.map_or((0, None), |(n, text)| (n, Some(text)));
                Response::Insert {
                    generation,
                    foreground: self.target,
                    backspaces,
                    text,
                }
            }
        }
    }
}
/// The typed prefix as it should read once the word is accepted. A capital
/// the word has where the person typed a lowercase letter is restored, as in
/// "lon" for London or "i" for I'm. A capital the person typed is kept.
/// `None` when nothing would change, which is the usual case and deletes
/// nothing.
fn prefix_offset(word: &str, typed: &str) -> Option<usize> {
    let normalized = switchify_prediction::normalize(typed);
    if normalized.is_empty() {
        return Some(0);
    }
    word.grapheme_indices(true)
        .map(|(i, g)| i + g.len())
        .find(|&end| switchify_prediction::normalize(&word[..end]) == normalized)
}

fn restored(typed: &str, head: &str) -> Option<String> {
    let merged: String = typed
        .graphemes(true)
        .zip(head.graphemes(true))
        .map(|(t, w)| {
            if t.chars().next().is_some_and(char::is_lowercase)
                && w.chars().next().is_some_and(char::is_uppercase)
            {
                t.to_uppercase()
            } else {
                t.to_owned()
            }
        })
        .collect();
    (merged != typed).then_some(merged)
}

/// A typed prefix of two or more letters, all capitals, is a word being
/// written in capitals: its completion continues that way whatever the
/// modifiers now say, so the label shows exactly what will be inserted.
fn shouting(prefix: &str) -> bool {
    let mut letters = prefix.chars().filter(|c| c.is_alphabetic());
    let first = letters.next();
    let second = letters.next();
    first.is_some_and(char::is_uppercase)
        && second.is_some_and(char::is_uppercase)
        && letters.all(char::is_uppercase)
}

/// The case a suggestion's suffix is inserted in. Caps, or Shift locked,
/// uppercases it all, and together they cancel like they do on typed letters.
/// With nothing typed yet, Shift once flips the first character's case the
/// way it would flip the next typed letter, and a sentence start capitalises
/// it without any modifier. After typed letters, a pending Shift once is left
/// for the next letter and does not touch the suggestion.
fn cased(suffix: &str, whole_word: bool, sentence_start: bool, shift: Shift, caps: bool) -> String {
    let upper = caps ^ (shift == Shift::Locked);
    let suffix = if upper {
        suffix.to_uppercase()
    } else {
        suffix.to_owned()
    };
    if !whole_word {
        return suffix;
    }
    let first_upper = if shift == Shift::Once {
        !upper
    } else {
        upper || sentence_start
    };
    // Only an explicit flip lowers a first letter; otherwise the model's own
    // casing, such as a name's capital, stays.
    let mut chars = suffix.chars();
    match chars.next() {
        Some(c) if first_upper => c.to_uppercase().collect::<String>() + chars.as_str(),
        Some(c) if shift == Shift::Once => c.to_lowercase().collect::<String>() + chars.as_str(),
        _ => suffix,
    }
}

/// Answers requests until the pipe closes. The observer starts on the first
/// request, which means a keyboard is open: a spare worker waiting for one
/// loads the model and observes nothing. Starting can take up to 500 ms of
/// the parent's two-second reply deadline, which holds because the first
/// request is sent before anything is typed and so runs no inference.
fn serve(
    engine: &mut Engine,
    input: &mut impl Read,
    output: &mut impl Write,
    start: impl FnOnce() -> bool,
) -> Result<(), ()> {
    let mut start = Some(start);
    while let Ok(request) = receive::<Request>(input) {
        if let Some(start) = start.take() {
            engine.tracked = start();
        }
        send(output, &engine.respond(request))?;
    }
    Ok(())
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
        activity::set_ignored(ignored);
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let database = Database::open(&path);
            let mut engine = Engine::new(database, false);
            serve(
                &mut engine,
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
                activity::start,
            )?;
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
    fn query(revision: u64, edits: Vec<RecordedEdit>) -> Vec<u8> {
        let mut frame = Vec::new();
        send(
            &mut frame,
            &Request::Query {
                generation: 0,
                edits,
                revision,
                shift: Shift::Off,
                caps: false,
                sentence_start: false,
            },
        )
        .unwrap();
        frame
    }
    #[test]
    fn the_observer_starts_once_on_the_first_request() {
        use std::cell::Cell;
        let started = Cell::new(0);
        let start = || {
            started.set(started.get() + 1);
            true
        };
        // A spare worker: the pipe closes before any request arrives.
        let mut idle = Engine::new(Database::fixture(), false);
        let mut output = Vec::new();
        serve(&mut idle, &mut std::io::Cursor::new([]), &mut output, start).unwrap();
        assert_eq!(started.get(), 0);
        assert!(!idle.tracked);
        assert!(output.is_empty());

        let mut e = engine();
        e.tracked = false;
        let input = [query(1, vec![append("wa")]), query(2, vec![append("t")])].concat();
        let mut output = Vec::new();
        serve(&mut e, &mut std::io::Cursor::new(input), &mut output, start).unwrap();
        assert_eq!(started.get(), 1);
        assert!(e.tracked);
        let mut replies = std::io::Cursor::new(output);
        for _ in 0..2 {
            assert!(matches!(
                receive::<Response>(&mut replies),
                Ok(Response::Suggestions { tracking: true, .. })
            ));
        }

        // An observer that cannot start leaves prediction untracked.
        let mut e = engine();
        let input = query(1, vec![]);
        let mut output = Vec::new();
        serve(
            &mut e,
            &mut std::io::Cursor::new(input),
            &mut output,
            || false,
        )
        .unwrap();
        assert!(!e.tracked);
    }
    #[test]
    fn normalized_prefixes_preserve_typed_graphemes_and_apostrophes() {
        assert_eq!(prefix_offset("café", "cafe\u{301}"), Some("café".len()));
        assert_eq!(prefix_offset("I'm", "i’"), Some(2));
        assert_eq!(prefix_offset("can't", "can’"), Some(4));
        assert_eq!(restored("i’", "I'").as_deref(), Some("I’"));
        assert_eq!(restored("cafe\u{301}", "café"), None);
        assert_eq!(prefix_offset("water", "wax"), None);
        assert_eq!(prefix_offset("water", ""), Some(0));
    }

    #[test]
    fn normalized_completions_insert_without_rewriting_typed_accents_or_apostrophes() {
        for (typed, expected, deletion, insertion) in [
            ("cafe\u{301}", "cafe\u{301}", 0, " "),
            ("can’", "can’t", 0, "t "),
            ("i’", "I’m", 2, "I’m "),
        ] {
            let mut e = engine();
            let b = e
                .query(vec![append(typed)], 1, Shift::Off, false, false)
                .unwrap();
            assert_eq!(b.words, vec![expected]);
            assert_eq!(e.accept(b.token, 0), Some((deletion, insertion.to_owned())));
        }
        let mut e = engine();
        assert!(e.query(vec![], 1, Shift::Off, false, false).is_none());
        assert!(!e
            .query(vec![append("I need ")], 2, Shift::Off, false, false)
            .unwrap()
            .words
            .is_empty());
    }

    #[test]
    fn five_slots_contain_ranked_words_only() {
        let mut e = engine();
        let b = e
            .query(vec![append("wa")], 1, Shift::Off, false, false)
            .unwrap();
        assert_eq!(b.words, vec!["water", "waffle", "walk"]);
        assert_eq!(e.accept(b.token, 1).unwrap(), (0, "ffle ".to_owned()));
        let upper = e.query(vec![], 1, Shift::Off, true, false).unwrap();
        assert_eq!(upper.words[1], "waFFLE");
    }
    #[test]
    fn first_letter_and_completion_chain_use_only_buffer() {
        let mut e = engine();
        let first = e
            .query(vec![append("w")], 1, Shift::Off, false, false)
            .unwrap();
        assert!(first.words.iter().any(|w| w == "water"));
        let b = e
            .query(vec![append("a")], 2, Shift::Off, false, false)
            .unwrap();
        assert_eq!(b.words[0], "water");
        let (deleted, suffix) = e.accept(b.token, 0).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(suffix, "ter ");
        assert!(e.accept(b.token, 0).is_none());
        e.query(vec![append(&suffix)], 3, Shift::Off, false, false);
        assert_eq!(e.buffer, "water ");
        e.query(
            vec![edit(Edit::Reset), append("wa")],
            4,
            Shift::Off,
            false,
            false,
        )
        .unwrap();
        assert_eq!(e.buffer, "wa");
    }
    #[test]
    fn delayed_observer_start_discards_unobserved_edits() {
        let mut e = engine();
        // A Switchify edit at 10 and external activity at 11 both precede
        // observer startup at 12. Only typing after startup is trustworthy.
        e.last_activity = || 12;
        e.query(vec![append("wa")], 1, Shift::Off, false, false);
        assert!(e.buffer.is_empty());
        let mut fresh = append("w");
        fresh.time = 13;
        assert!(e.query(vec![fresh], 2, Shift::Off, false, false).is_some());
        assert_eq!(e.buffer, "w");
    }
    #[test]
    fn queued_edits_are_scoped_to_window_and_external_activity() {
        let mut e = engine();
        let b = e
            .query(vec![append("wa")], 1, Shift::Off, false, false)
            .unwrap();
        e.observe = || (1, true);
        e.last_activity = || 11;
        assert!(e.accept(b.token, 0).is_none());
        let mut fresh = append("he");
        fresh.time = 12;
        e.query(vec![append("ter"), fresh], 2, Shift::Off, false, false);
        assert_eq!(e.buffer, "he");
        e.foreground = || Ok(2);
        e.query(vec![append("wa")], 3, Shift::Off, false, false);
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
            Shift::Off,
            false,
            false,
        );
        assert_eq!(e.buffer, "wa");
        for rev in 2..10 {
            e.query(
                vec![append(&" word".repeat(90))],
                rev,
                Shift::Off,
                false,
                false,
            );
        }
        assert!(e.buffer.chars().count() <= 512);
        assert!(e.clipped);
        e.query(vec![edit(Edit::Reset)], 10, Shift::Off, false, false);
        assert!(e.buffer.is_empty());
        e.query(vec![append("wa"); 513], 11, Shift::Off, false, false);
        assert!(e.buffer.is_empty());
    }
    #[test]
    fn stale_revisions_health_and_foreground_races_reject_acceptance() {
        let mut e = engine();
        let b = e
            .query(vec![append("wa")], 1, Shift::Off, false, false)
            .unwrap();
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
        e.query(vec![append("wa")], 2, Shift::Off, false, false);
        assert!(e
            .query(vec![append("wa")], 2, Shift::Off, false, false)
            .is_none());
        let b = e
            .query(vec![append("wa")], 3, Shift::Off, false, false)
            .unwrap();
        e.observe = || (0, false);
        assert!(e.accept(b.token, 0).is_none());
        assert!(e
            .query(vec![append("wa")], 4, Shift::Off, false, false)
            .is_none());
    }
    #[test]
    fn casing_and_unchanged_context_keep_choice_identity() {
        let mut e = engine();
        let b = e
            .query(vec![append("Wa")], 1, Shift::Off, false, false)
            .unwrap();
        assert_eq!(b.words[0], "Water");
        assert_eq!(
            e.query(vec![], 1, Shift::Off, false, false).unwrap().token,
            b.token
        );
        let upper = e.query(vec![], 1, Shift::Off, true, false).unwrap();
        assert_eq!(upper.words[0], "WaTER");
        assert_ne!(upper.token, b.token);
    }
    #[test]
    fn accepting_restores_a_capital_the_typed_prefix_lacks_and_nothing_else() {
        let w = |s: &[&str]| s.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(restored("lon", "Lon").as_deref(), Some("Lon"));
        assert_eq!(restored("i’", "I’").as_deref(), Some("I’"));
        assert_eq!(restored("whats", "Whats").as_deref(), Some("Whats"));
        assert_eq!(restored("Wa", "wa"), None);
        assert_eq!(restored("wa", "wa"), None);
        assert_eq!(restored("", ""), None);
        let mut e = engine();
        // The label reads as the text will, and only the prefix is deleted.
        let b = e
            .query(vec![append("wh")], 1, Shift::Off, false, false)
            .unwrap();
        assert_eq!(b.words, w(&["WhatsApp"]));
        assert_eq!(e.accept(b.token, 0).unwrap(), (2, "WhatsApp ".to_owned()));
        // The edits the service queues afterwards leave the buffer reading
        // as the screen does.
        e.query(
            vec![
                edit(Edit::Backspace),
                edit(Edit::Backspace),
                append("WhatsApp "),
            ],
            2,
            Shift::Off,
            false,
            false,
        );
        assert_eq!(e.buffer, "WhatsApp ");
        // A capital the person typed is kept, and nothing is deleted.
        let b = e
            .query(
                vec![edit(Edit::Reset), append("Wh")],
                3,
                Shift::Off,
                false,
                false,
            )
            .unwrap();
        assert_eq!(b.words, w(&["WhatsApp"]));
        assert_eq!(e.accept(b.token, 0).unwrap(), (0, "atsApp ".to_owned()));
        // A word in capitals is never retyped.
        let b = e
            .query(
                vec![edit(Edit::Reset), append("WH")],
                4,
                Shift::Off,
                false,
                false,
            )
            .unwrap();
        assert_eq!(e.accept(b.token, 0).unwrap(), (0, "ATSAPP ".to_owned()));
        match e.respond(Request::Accept {
            generation: 0,
            token: 0,
            revision: 99,
            index: 0,
        }) {
            Response::Insert {
                backspaces, text, ..
            } => {
                assert_eq!((backspaces, text), (0, None));
            }
            _ => panic!("expected an insert response"),
        }
    }
    #[test]
    fn an_all_caps_prefix_continues_in_capitals_whatever_the_modifiers() {
        let mut e = engine();
        e.query(vec![append("WA")], 1, Shift::Off, false, false)
            .unwrap();
        let row = e.query(vec![], 1, Shift::Off, false, false).unwrap();
        assert_eq!(row.words[0], "WATER");
        assert_eq!(row.words[1], "WAFFLE");
        assert_eq!(
            e.query(vec![], 1, Shift::Locked, false, false)
                .unwrap()
                .words[0],
            "WATER"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Off, true, false).unwrap().words[0],
            "WATER"
        );
        // One capital is a capitalised word, not a word in capitals.
        e.query(
            vec![edit(Edit::Reset), append("W")],
            2,
            Shift::Off,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            e.query(vec![], 2, Shift::Off, false, false).unwrap().words[0],
            "Water"
        );
    }
    #[test]
    fn the_keyboard_alone_decides_a_sentence_start() {
        let mut e = engine();
        // Punctuation in the buffer is not enough: the keyboard may have seen
        // the person cancel the capital, or the buffer may have started
        // mid-line after a reset.
        let buffer_only = e
            .query(vec![append("Done! ")], 1, Shift::Off, false, false)
            .unwrap();
        assert_eq!(buffer_only.words[0], "water");
        let suggestions = e.query(vec![], 1, Shift::Off, false, true).unwrap();
        assert_eq!(suggestions.words[0], "Water");
        let locked = e.query(vec![], 1, Shift::Locked, false, true).unwrap();
        assert_eq!(locked.words[0], "WATER");
        let once = e.query(vec![], 1, Shift::Once, false, true).unwrap();
        assert_eq!(once.words[0], "Water");
        // Modifiers still mirror typing at a sentence start: Caps with Shift
        // locked cancel to lowercase letters, and Shift once under Caps
        // lowers only the first one, exactly as the keys would type them.
        assert_eq!(
            e.query(vec![], 1, Shift::Off, true, true).unwrap().words[0],
            "WATER"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Locked, true, true).unwrap().words[0],
            "Water"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Once, true, true).unwrap().words[0],
            "wATER"
        );
    }
    #[test]
    fn shift_once_changes_only_the_first_letter_and_only_before_typing() {
        let mut e = engine();
        e.query(vec![append("Send ")], 1, Shift::Off, false, false)
            .unwrap();
        assert_eq!(
            e.query(vec![], 1, Shift::Off, false, false).unwrap().words[0],
            "water"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Once, false, false).unwrap().words[0],
            "Water"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Locked, false, false)
                .unwrap()
                .words[0],
            "WATER"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Off, true, false).unwrap().words[0],
            "WATER"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Locked, true, false)
                .unwrap()
                .words[0],
            "water"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Once, true, false).unwrap().words[0],
            "wATER"
        );
        assert_eq!(
            e.query(vec![], 1, Shift::Off, false, false).unwrap().words[3],
            "WhatsApp"
        );
        e.query(vec![append("wa")], 2, Shift::Off, false, false)
            .unwrap();
        assert_eq!(
            e.query(vec![], 2, Shift::Once, false, false).unwrap().words[0],
            "water"
        );
        assert_eq!(
            e.query(vec![], 2, Shift::Locked, false, false)
                .unwrap()
                .words[0],
            "waTER"
        );
    }
    /// Every combination of typed prefix, Shift, Caps and sentence start for
    /// one word, then the name cases with Caps off. The expected label and
    /// insert are written out by hand, so the table documents the behaviour
    /// rather than restating the rule.
    #[test]
    fn casing_matrix() {
        use Shift::{Locked, Off, Once};
        const EITHER: [bool; 2] = [false, true];
        // (typed, shift, caps, sentence starts, label, deleted, typed text)
        type Row = (
            &'static str,
            Shift,
            bool,
            &'static [bool],
            &'static str,
            usize,
            &'static str,
        );
        let rows: &[Row] = &[
            // Nothing of the word typed yet: the whole word is cased.
            ("", Off, false, &[false], "water", 0, "water "),
            ("", Off, false, &[true], "Water", 0, "Water "),
            ("", Once, false, &EITHER, "Water", 0, "Water "),
            ("", Locked, false, &EITHER, "WATER", 0, "WATER "),
            ("", Off, true, &EITHER, "WATER", 0, "WATER "),
            ("", Once, true, &EITHER, "wATER", 0, "wATER "),
            ("", Locked, true, &[false], "water", 0, "water "),
            ("", Locked, true, &[true], "Water", 0, "Water "),
            // Lowercase letters typed: modifiers case the rest only, and a
            // pending Shift once is left for the next typed letter.
            ("wa", Off, false, &EITHER, "water", 0, "ter "),
            ("wa", Once, false, &EITHER, "water", 0, "ter "),
            ("wa", Locked, false, &EITHER, "waTER", 0, "TER "),
            ("wa", Off, true, &EITHER, "waTER", 0, "TER "),
            ("wa", Once, true, &EITHER, "waTER", 0, "TER "),
            ("wa", Locked, true, &EITHER, "water", 0, "ter "),
            // A capital the person typed is kept.
            ("Wa", Off, false, &EITHER, "Water", 0, "ter "),
            ("Wa", Once, false, &EITHER, "Water", 0, "ter "),
            ("Wa", Locked, false, &EITHER, "WaTER", 0, "TER "),
            ("Wa", Off, true, &EITHER, "WaTER", 0, "TER "),
            ("Wa", Once, true, &EITHER, "WaTER", 0, "TER "),
            ("Wa", Locked, true, &EITHER, "Water", 0, "ter "),
            // A word typed in capitals continues in capitals.
            ("WA", Off, false, &EITHER, "WATER", 0, "TER "),
            ("WA", Once, false, &EITHER, "WATER", 0, "TER "),
            ("WA", Locked, false, &EITHER, "WATER", 0, "TER "),
            ("WA", Off, true, &EITHER, "WATER", 0, "TER "),
            ("WA", Once, true, &EITHER, "WATER", 0, "TER "),
            ("WA", Locked, true, &EITHER, "WATER", 0, "TER "),
            // A name restores its capital by retyping a lowercase prefix,
            // keeps a typed capital, and is never retyped in capitals.
            ("wh", Off, false, &EITHER, "WhatsApp", 2, "WhatsApp "),
            ("wh", Once, false, &EITHER, "WhatsApp", 2, "WhatsApp "),
            ("Wh", Off, false, &EITHER, "WhatsApp", 0, "atsApp "),
            ("WH", Off, false, &EITHER, "WHATSAPP", 0, "ATSAPP "),
        ];
        let mut e = engine();
        let mut revision = 0;
        let mut typed = None;
        for &(prefix, shift, caps, starts, label, deleted, text) in rows {
            if typed != Some(prefix) {
                typed = Some(prefix);
                revision += 1;
                // "Send " leaves a buffer whose current word is empty.
                let buffer = format!("Send {prefix}");
                e.query(
                    vec![edit(Edit::Reset), append(&buffer)],
                    revision,
                    Off,
                    false,
                    false,
                );
            }
            for &sentence_start in starts {
                let case = format!("{prefix:?} {shift:?} caps={caps} start={sentence_start}");
                let row = e
                    .query(vec![], revision, shift, caps, sentence_start)
                    .unwrap_or_else(|| panic!("no suggestions for {case}"));
                assert_eq!(row.words[0], label, "label for {case}");
                assert_eq!(
                    e.accept(row.token, 0),
                    Some((deleted, text.to_owned())),
                    "insert for {case}"
                );
            }
        }
    }
    #[test]
    fn private_frames_are_bounded() {
        assert!(receive::<Request>(&mut &b"bad!"[..]).is_err());
        let request = Request::Query {
            generation: 0,
            revision: 1,
            edits: vec![append(&"x".repeat(LIMIT))],
            shift: Shift::Off,
            caps: false,
            sentence_start: false,
        };
        assert!(send(&mut Vec::new(), &request).is_err());
    }
}
