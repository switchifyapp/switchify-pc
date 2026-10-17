//! English word prediction, with optional personal learning. Only the
//! isolated worker holds context.
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use switchify_prediction::{Options, Predictor};

const DATABASE_SHA256: &str = "c6df388c845354a059333b1e4abe68719e5a7016e8514c44581e14ed16bf964a";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prediction {
    pub words: [Option<String>; SLOTS],
}

#[cfg(test)]
pub fn slots(words: impl IntoIterator<Item = impl Into<String>>) -> [Option<String>; SLOTS] {
    let mut words = words.into_iter();
    std::array::from_fn(|_| words.next().map(Into::into))
}

pub trait Predict: Send {
    #[cfg(test)]
    fn neural_status(&self) -> Option<switchify_prediction_neural::Status> {
        None
    }
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()>;
    fn poll(&mut self) -> Option<Prediction> {
        None
    }
    fn pending(&self) -> bool {
        false
    }
    fn reset(&mut self) {}
    /// Learn one completed, non-overlapping piece of typed text.
    fn learn(&mut self, _text: &str) {}
}

/// Prediction slots: three instant words, two generated words, then three
/// generated phrases.
pub const SLOTS: usize = 8;
/// The slots that hold generated two-word phrases. Words never take them.
pub const PHRASE_SLOTS: std::ops::Range<usize> = 5..SLOTS;
/// Generated words fill slots 4 and 5 at most, leaving the phrase slots.
const GENERATED_WORDS: usize = 2;

/// Generation for the latest prediction. The companion runs one request at a
/// time, so the phrases are requested once the words have arrived.
#[derive(Default)]
struct Refining {
    before: String,
    prefix: String,
    words: Option<u64>,
    phrases: Option<u64>,
    generated: Vec<String>,
}

pub struct Model {
    predictor: Predictor,
    neural: Option<switchify_prediction_neural::Refiner>,
    refining: Refining,
    instant: [Option<String>; 3],
    config: Option<switchify_prediction_neural::Config>,
    learning: bool,
}

impl Model {
    /// Opens the verified baseline, and the personal file when given. A
    /// personal file that cannot be opened leaves prediction working without
    /// learning.
    pub fn open(path: &Path, personal: Option<&Path>) -> Result<Self, ()> {
        let mut file = File::open(path).map_err(|_| ())?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let n = file.read(&mut buffer).map_err(|_| ())?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        if format!("{:x}", hash.finalize()) != DATABASE_SHA256 {
            return Err(());
        }
        let (predictor, learning) = match personal.map(|p| Predictor::open(path, Some(p))) {
            Some(Ok(predictor)) => (predictor, true),
            _ => (Predictor::open(path, None).map_err(|_| ())?, false),
        };
        let config = (|| {
            let bundle = path.parent()?.parent()?.join("prediction-neural");
            let executable = std::env::current_exe().ok()?;
            #[cfg(test)]
            let executable = std::env::var_os("SWITCHIFY_BENCHMARK_WORKER")
                .map(std::path::PathBuf::from)
                .unwrap_or(executable);
            let worker = |name: &str| {
                let name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
                let bundled = executable.parent()?.join(&name);
                if bundled.is_file() {
                    return Some(bundled);
                }
                if cfg!(debug_assertions) {
                    let target = if cfg!(target_os = "windows") {
                        "x86_64-pc-windows-msvc"
                    } else if cfg!(target_arch = "aarch64") {
                        "aarch64-apple-darwin"
                    } else {
                        "x86_64-apple-darwin"
                    };
                    return Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries").join(
                        format!(
                            "{}-{target}{}",
                            name.trim_end_matches(std::env::consts::EXE_SUFFIX),
                            std::env::consts::EXE_SUFFIX
                        ),
                    ));
                }
                None
            };
            Some(switchify_prediction_neural::Config {
                bundle,
                portable_worker: worker("switchify-smol-worker")?,
                accelerated_worker: if cfg!(target_os = "windows") {
                    worker("switchify-smol-worker-avx2").filter(|p| p.is_file())
                } else {
                    None
                },
                threads: 4,
            })
        })();
        Ok(Self {
            predictor,
            neural: None,
            config,
            refining: Refining::default(),
            instant: Default::default(),
            learning,
        })
    }
}

fn display_word(word: String) -> String {
    match word.as_str() {
        "i" | "i'm" | "i'll" | "i'd" | "i've" => "I".to_owned() + &word[1..],
        _ => word,
    }
}

/// Generated words follow the instant ones, taking any slot they left free
/// before the phrase slots. Phrases take the phrase slots in order.
fn merged(
    instant: &[Option<String>; 3],
    generated: &[String],
    phrases: Vec<Vec<String>>,
) -> [Option<String>; SLOTS] {
    let mut words = instant.iter().flatten().cloned().chain(
        generated
            .iter()
            .take(GENERATED_WORDS)
            .cloned()
            .map(display_word),
    );
    let mut phrases = phrases.into_iter().map(|words| {
        words
            .into_iter()
            .map(display_word)
            .collect::<Vec<_>>()
            .join(" ")
    });
    std::array::from_fn(|i| {
        if i < PHRASE_SLOTS.start {
            words.next()
        } else {
            phrases.next()
        }
    })
}
impl Predict for Model {
    #[cfg(test)]
    fn neural_status(&self) -> Option<switchify_prediction_neural::Status> {
        self.neural.as_ref().map(|n| n.status())
    }
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        // Child inference starts only after the parent has contained this worker
        // and sent a query, never during speculative statistical loading.
        if let Some(config) = self.config.take() {
            self.neural = switchify_prediction_neural::Refiner::new(config).ok();
        }
        let options = Options {
            limit: 3,
            min_chars: 0,
            unigram_only: false,
        };
        let words: Vec<String> = self
            .predictor
            .predict(before, prefix, options)
            .into_iter()
            .map(|s| s.word)
            .collect();
        self.instant = std::array::from_fn(|i| words.get(i).cloned().map(display_word));
        self.refining = Refining {
            before: before.to_owned(),
            prefix: prefix.to_owned(),
            words: self.neural.as_mut().and_then(|n| {
                n.generate(before, prefix, 0, &words, GENERATED_WORDS)
                    .ok()
                    .flatten()
            }),
            ..Refining::default()
        };
        Ok(Prediction {
            words: std::array::from_fn(|i| self.instant.get(i).cloned().flatten()),
        })
    }
    fn poll(&mut self) -> Option<Prediction> {
        let neural = self.neural.as_mut()?;
        let result = neural.poll()?;
        let refining = &mut self.refining;
        let phrases = if refining.words == Some(result.request_id) {
            refining.words = None;
            refining.generated = result.words;
            // Words first, then the phrases: a new request replaces any other.
            refining.phrases = neural
                .generate_phrases(
                    &refining.before,
                    &refining.prefix,
                    0,
                    &[],
                    PHRASE_SLOTS.len(),
                )
                .ok()
                .flatten();
            Vec::new()
        } else if refining.phrases == Some(result.request_id) {
            refining.phrases = None;
            result.phrases
        } else {
            return None;
        };
        Some(Prediction {
            words: merged(&self.instant, &refining.generated, phrases),
        })
    }
    fn pending(&self) -> bool {
        (self.refining.words.is_some() || self.refining.phrases.is_some())
            && self.neural.as_ref().is_some_and(|n| {
                matches!(
                    n.status(),
                    switchify_prediction_neural::Status::Ready
                        | switchify_prediction_neural::Status::Loading
                )
            })
    }
    fn reset(&mut self) {
        self.refining = Refining::default();
        self.instant = Default::default();
        if let Some(n) = &mut self.neural {
            n.reset();
        }
    }
    fn learn(&mut self, text: &str) {
        // A failed write keeps the previous snapshot; prediction carries on.
        if self.learning {
            let _ = self.predictor.learn(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::Instant};

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("switchify-prediction-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn generated_words_take_the_next_free_slot_and_phrases_their_own() {
        let instant = |words: &[&str]| std::array::from_fn(|i| words.get(i).map(|w| w.to_string()));
        let generated = ["i'm".to_owned(), "here".to_owned(), "home".to_owned()];
        let phrase = |a: &str, b: &str| vec![a.to_owned(), b.to_owned()];
        let phrases = || {
            vec![
                phrase("i", "am"),
                phrase("i", "will"),
                phrase("it", "is"),
                phrase("is", "extra"),
            ]
        };
        assert_eq!(
            merged(&instant(&["help"]), &generated, Vec::new()),
            slots(["help", "I'm", "here"])
        );
        assert_eq!(
            merged(&instant(&[]), &generated, Vec::new()),
            slots(["I'm", "here"])
        );
        assert_eq!(
            merged(&instant(&["a", "b", "c"]), &generated, phrases()),
            slots(["a", "b", "c", "I'm", "here", "I am", "I will", "it is"])
        );
        let mut expected = slots(["a", "I'm", "here"]);
        expected[PHRASE_SLOTS.start] = Some("I am".into());
        expected[PHRASE_SLOTS.start + 1] = Some("I will".into());
        expected[PHRASE_SLOTS.start + 2] = Some("it is".into());
        assert_eq!(merged(&instant(&["a"]), &generated, phrases()), expected);
        let mut expected: [Option<String>; SLOTS] = Default::default();
        expected[PHRASE_SLOTS.start] = Some("going to".into());
        assert_eq!(
            merged(&instant(&[]), &[], vec![phrase("going", "to")]),
            expected
        );
    }

    #[test]
    fn learning_writes_only_the_personal_file_and_ranks_learned_words() {
        let dir = Fixture::new();
        let path = dir.0.join("english.sqlite");
        let personal = dir.0.join("personal.sqlite");
        switchify_prediction::build(
            &path,
            "I need help. I need help. I drink water.",
            "synthetic test",
        )
        .unwrap();
        let original = fs::read(&path).unwrap();
        let open = |learning| Model {
            predictor: Predictor::open(&path, Some(&personal)).unwrap(),
            neural: None,
            config: None,
            refining: Refining::default(),
            instant: Default::default(),
            learning,
        };
        let first = |model: &mut Model| model.predict("I need ", "h").unwrap().words[0].clone();
        let mut model = open(false);
        model.learn("I need hummus. I need hummus.");
        assert_eq!(first(&mut model).as_deref(), Some("help"));
        let mut model = open(true);
        // A word the baseline lacks counts once learned twice.
        model.learn("I need hummus.");
        assert_eq!(first(&mut model).as_deref(), Some("help"));
        model.learn("I need hummus.");
        assert_eq!(first(&mut model).as_deref(), Some("hummus"));
        // Learning persists for the next worker.
        assert_eq!(first(&mut open(true)).as_deref(), Some("hummus"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn sqlite_adapter_ranks_context_and_preserves_read_only_baseline() {
        let dir = Fixture::new();
        let path = dir.0.join("english.sqlite");
        switchify_prediction::build(&path,
            "I need help. I need help. I need help. I drink water. I drink water. Café can't wait. I'm here. I am home. I am happy. I am healthy. I am hungry. I am hopeful. I am human.", "synthetic test").unwrap();
        let original = fs::read(&path).unwrap();
        let mut model = Model {
            predictor: Predictor::open(&path, None).unwrap(),
            neural: None,
            config: None,
            refining: Refining::default(),
            instant: Default::default(),
            learning: false,
        };
        model.config = Some(switchify_prediction_neural::Config {
            bundle: dir.0.join("missing"),
            portable_worker: dir.0.join("missing-worker"),
            accelerated_worker: None,
            threads: 4,
        });
        assert_eq!(
            model.predict("I need ", "h").unwrap().words[0]
                .as_deref()
                .unwrap(),
            "help"
        );
        assert!(!model.pending());
        assert!(model.config.is_none());
        assert!(model.poll().is_none());
        assert_eq!(
            model.predict("I drink ", "").unwrap().words[0]
                .as_deref()
                .unwrap(),
            "water"
        );
        assert!(model
            .predict("", "zyzzy")
            .unwrap()
            .words
            .iter()
            .all(Option::is_none));
        assert_eq!(
            model.predict("", "cafe\u{301}").unwrap().words,
            crate::prediction::slots(["café"])
        );
        assert_eq!(
            model.predict("", "can’").unwrap().words,
            crate::prediction::slots(["can't"])
        );
        assert_eq!(
            model.predict("", "i’").unwrap().words,
            crate::prediction::slots(["I'm"])
        );
        assert_eq!(
            model
                .predict("", "h")
                .unwrap()
                .words
                .iter()
                .flatten()
                .count(),
            3
        );
        assert_eq!(
            model.predict("I need.", "h").unwrap(),
            model.predict("", "h").unwrap()
        );
        drop(model);
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        // A structurally valid custom database is not the pinned production model.
        assert!(Model::open(&path, None).is_err());
        fs::write(&path, "corrupt").unwrap();
        assert!(Model::open(&path, None).is_err());
        assert!(Model::open(&dir.0.join("missing"), None).is_err());
    }

    fn rss_bytes() -> u64 {
        let pid = std::process::id().to_string();
        #[cfg(target_os = "windows")]
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).WorkingSet64"),
            ])
            .output()
            .unwrap();
        #[cfg(not(target_os = "windows"))]
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid])
            .output()
            .unwrap();
        assert!(output.status.success());
        let value: u64 = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        if cfg!(target_os = "windows") {
            value
        } else {
            value * 1024
        }
    }

    #[test]
    #[ignore = "isolated measurement; run with --ignored --test-threads=1"]
    fn bundled_prediction_benchmark() {
        let path = std::env::var_os("SWITCHIFY_BENCHMARK_MODEL")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("resources/prediction-model/english.ngrams")
            });
        let before = rss_bytes();
        let start = Instant::now();
        let mut model = Model::open(&path, None).unwrap();
        let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
        let loaded = rss_bytes();
        let mut samples = Vec::new();
        for i in 0..220 {
            let (context, prefix) = [
                ("I need ", "he"),
                ("I want ", ""),
                ("", "a"),
                ("can you ", "h"),
            ][i % 4];
            let start = Instant::now();
            let suggestions = model.predict(context, prefix).unwrap();
            assert!(!suggestions.words.iter().all(Option::is_none));
            if i >= 20 {
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        samples.sort_by(f64::total_cmp);
        assert_eq!(
            model.predict("I need ", "he").unwrap().words[0]
                .as_deref()
                .unwrap(),
            "help"
        );
        #[cfg(target_os = "macos")]
        let cpu = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .unwrap();
        #[cfg(target_os = "windows")]
        let cpu = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_Processor).Name",
            ])
            .output()
            .unwrap();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let cpu = std::process::Command::new("uname")
            .arg("-m")
            .output()
            .unwrap();
        assert!(cpu.status.success());
        let report = serde_json::json!({
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "logical_cpus": std::thread::available_parallelism().unwrap().get(),
            "cpu": String::from_utf8_lossy(&cpu.stdout).trim(),
            "cold_load_ms": cold_ms, "warm_p50_ms": samples[99], "warm_p95_ms": samples[189],
            "rss_before_bytes": before, "rss_loaded_bytes": loaded,
            "rss_increase_bytes": loaded.saturating_sub(before),
            "memory_note": "Isolated Rust test process using the production adapter, including test harness; not total desktop RSS.",
            "database_bytes": fs::metadata(path).unwrap().len(), "samples": samples.len(),
        });
        println!("{report}");
        if let Some(path) = std::env::var_os("SWITCHIFY_BENCHMARK_REPORT") {
            fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
    }
}
