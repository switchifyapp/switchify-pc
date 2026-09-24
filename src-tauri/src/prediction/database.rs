use super::{
    context::Context,
    lookup::Lookup,
    model::{Model, Predict},
};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

/// The model stops exploring new words after this long.
const SEARCH_BUDGET: Duration = Duration::from_millis(400);
/// A model call slower than this switches the worker to the lookup for good,
/// keeping replies well inside the main process's two-second deadline.
const SLOW_CALL: Duration = Duration::from_millis(1000);

enum Enhanced {
    Loading(mpsc::Receiver<Result<Model, ()>>),
    Ready(Box<dyn Predict>),
    Unavailable,
}

// The lookup answers until the optional model has loaded in the background,
// and whenever the model fails or runs slowly, so it never delays typing.
pub struct Database {
    lookup: Lookup,
    enhanced: Enhanced,
    slow_call: Duration,
}
impl Database {
    pub fn open(path: &Path, enhanced: bool) -> Result<Self, ()> {
        let lookup = Lookup::open(path)?;
        let enhanced = match path.parent().filter(|_| enhanced) {
            Some(resources) => {
                let model = resources.join("prediction-model");
                let blocklist = resources.join("prediction-blocklist.txt");
                let (tx, rx) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let _ = tx.send(Model::open(&model, &blocklist));
                });
                Enhanced::Loading(rx)
            }
            None => Enhanced::Unavailable,
        };
        Ok(Self {
            lookup,
            enhanced,
            slow_call: SLOW_CALL,
        })
    }
    pub fn predict(&mut self, context: &Context) -> Result<Vec<String>, ()> {
        let base = self.lookup.predict(&context.words, &context.prefix);
        if let Enhanced::Loading(rx) = &self.enhanced {
            self.enhanced = match rx.try_recv() {
                Ok(Ok(model)) => Enhanced::Ready(Box::new(model)),
                Err(mpsc::TryRecvError::Empty) => return Ok(base),
                _ => Enhanced::Unavailable,
            };
        }
        let Enhanced::Ready(model) = &mut self.enhanced else {
            return Ok(base);
        };
        if context.partial {
            return Ok(base);
        }
        let lookup = &self.lookup;
        let known = |w: &str| lookup.spelling(w).map(str::to_owned);
        let start = Instant::now();
        let result = model.predict(
            &context.before,
            &context.prefix,
            &known,
            start + SEARCH_BUDGET,
        );
        if start.elapsed() > self.slow_call {
            self.enhanced = Enhanced::Unavailable;
        }
        match result {
            Ok(mut words) => {
                for word in base {
                    if words.len() < 5
                        && !words
                            .iter()
                            .any(|w| w.to_lowercase() == word.to_lowercase())
                    {
                        words.push(word);
                    }
                }
                words.truncate(5);
                Ok(words)
            }
            Err(()) => Ok(base),
        }
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            lookup: Lookup::fixture(),
            enhanced: Enhanced::Unavailable,
            slow_call: SLOW_CALL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Result<Vec<&'static str>, ()>, Duration);
    impl Predict for Fake {
        fn predict(
            &mut self,
            _before: &str,
            _prefix: &str,
            _known: &dyn Fn(&str) -> Option<String>,
            _deadline: Instant,
        ) -> Result<Vec<String>, ()> {
            std::thread::sleep(self.1);
            self.0
                .clone()
                .map(|w| w.into_iter().map(String::from).collect())
        }
    }
    fn ready(db: &mut Database, words: Result<Vec<&'static str>, ()>, delay: Duration) {
        db.enhanced = Enhanced::Ready(Box::new(Fake(words, delay)));
    }
    #[test]
    fn model_words_lead_and_the_lookup_fills_without_duplicates() {
        let ctx = super::super::context::extract("I would like wa", false);
        let mut db = Database::fixture();
        let lookup = db.predict(&ctx).unwrap();
        assert!(lookup.len() >= 2);
        ready(&mut db, Ok(vec!["WhatsApp", "WATER"]), Duration::ZERO);
        let words = db.predict(&ctx).unwrap();
        assert_eq!(words[..2], ["WhatsApp", "WATER"]);
        assert!(!words[2..].iter().any(|w| w.eq_ignore_ascii_case("water")));
        assert!(words.len() <= 5);
        ready(
            &mut db,
            Ok(vec!["a", "b", "c", "d", "e", "f"]),
            Duration::ZERO,
        );
        assert_eq!(db.predict(&ctx).unwrap(), ["a", "b", "c", "d", "e"]);
        ready(&mut db, Err(()), Duration::ZERO);
        assert_eq!(db.predict(&ctx).unwrap(), lookup);
        assert!(matches!(db.enhanced, Enhanced::Ready(_)));
    }
    #[test]
    fn slow_models_and_clipped_fragments_use_the_lookup() {
        let mut db = Database::fixture();
        let clipped = super::super::context::extract("agmentwa", true);
        let lookup = db.predict(&clipped).unwrap();
        ready(&mut db, Ok(vec!["model"]), Duration::ZERO);
        assert_eq!(db.predict(&clipped).unwrap(), lookup);
        let ctx = super::super::context::extract("I would like wa", false);
        db.slow_call = Duration::from_millis(20);
        ready(&mut db, Ok(vec!["model"]), Duration::from_millis(40));
        assert_eq!(db.predict(&ctx).unwrap()[0], "model");
        assert!(matches!(db.enhanced, Enhanced::Unavailable));
        assert_ne!(db.predict(&ctx).unwrap()[0], "model");
    }
    #[test]
    fn lookup_answers_while_the_model_loads_or_after_it_fails() {
        let ctx = super::super::context::extract("I would like wa", false);
        let mut db = Database::fixture();
        let expected = db.predict(&ctx).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        db.enhanced = Enhanced::Loading(rx);
        assert_eq!(db.predict(&ctx).unwrap(), expected);
        assert!(matches!(db.enhanced, Enhanced::Loading(_)));
        tx.send(Err(())).unwrap();
        assert_eq!(db.predict(&ctx).unwrap(), expected);
        assert!(matches!(db.enhanced, Enhanced::Unavailable));
        let (tx, rx) = mpsc::sync_channel::<Result<Model, ()>>(1);
        db.enhanced = Enhanced::Loading(rx);
        drop(tx);
        assert_eq!(db.predict(&ctx).unwrap(), expected);
        assert!(matches!(db.enhanced, Enhanced::Unavailable));
    }
    #[test]
    fn a_missing_model_leaves_the_lookup_working() {
        let dir = std::env::temp_dir().join(format!("switchify-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lookup = dir.join("word-predictions.lookup");
        std::fs::write(&lookup, Lookup::fixture_bytes()).unwrap();
        let mut db = Database::open(&lookup, true).unwrap();
        let ctx = super::super::context::extract("I would like wa", false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while matches!(db.enhanced, Enhanced::Loading(_)) && std::time::Instant::now() < deadline {
            assert!(!db.predict(&ctx).unwrap().is_empty());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(db.enhanced, Enhanced::Unavailable));
        assert!(!db.predict(&ctx).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bundled_lookup_matches_sqlite_corpus_and_expanded_samples() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let source = root.join("WordData2017051601.db");
        let reference = super::super::database_reference::Database::open(&source).unwrap();
        let mut lookup = Database::open(&root.join("word-predictions.lookup"), false).unwrap();
        let c = rusqlite::Connection::open_with_flags(
            source,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut texts: Vec<String> = [
            "",
            "a",
            "w",
            "wa",
            "water",
            "zzzzzz",
            "I ",
            "I would ",
            "I would like ",
            "I would like wa",
            "the ",
            "the a",
            "unknownword wa",
            "café",
            "cafe\u{301}",
            "İ",
            "你好",
            "I can’t find ",
            "Done! ",
            "hello\nwa",
            "W",
            "WATER",
            "🙂 ",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ids: std::collections::HashMap<u32, String> = c
            .prepare("SELECT ID,WORD FROM WORDS")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for offset in [0, 499] {
            for (n, table) in [(2, "BIGRAMS"), (3, "TRIGRAMS"), (4, "QUADGRAMS")] {
                let fields = (1..=n)
                    .map(|i| format!("ID{i}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let mut q = c
                    .prepare(&format!(
                        "SELECT {fields} FROM {table} WHERE ID%997={offset} ORDER BY ID LIMIT 80"
                    ))
                    .unwrap();
                let rows = q
                    .query_map([], |r| {
                        (0..n)
                            .map(|i| r.get::<_, u32>(i))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap();
                for row in rows {
                    let row = row.unwrap();
                    let context = row[..n - 1]
                        .iter()
                        .map(|id| ids[id].as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    for size in [0, 1, 3] {
                        texts.push(format!(
                            "{context} {}",
                            ids[&row[n - 1]].chars().take(size).collect::<String>()
                        ));
                    }
                }
            }
        }
        assert_eq!(texts.len(), 1463);
        for (i, text) in texts.iter().enumerate() {
            let context = super::super::context::extract(text, i % 17 == 0);
            assert_eq!(
                lookup.predict(&context),
                reference.predict(&context),
                "case {i}"
            );
        }
    }
}
