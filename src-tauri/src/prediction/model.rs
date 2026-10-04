//! Read-only English word prediction. Only the isolated worker holds context.
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use switchify_prediction::{Options, Predictor};

const DATABASE_SHA256: &str = "222253417d0a7a705823ffb7e599a3bcf5d5d3daf4a9d76161ac6b3e555aeaad";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prediction {
    pub words: Vec<String>,
}

pub trait Predict: Send {
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()>;
    fn poll(&mut self) -> Option<Prediction> {
        None
    }
    fn pending(&self) -> bool {
        false
    }
    fn reset(&mut self) {}
}

pub struct Model {
    predictor: Predictor,
    neural: Option<switchify_prediction_neural::Refiner>,
    request: Option<u64>,
    config: Option<switchify_prediction_neural::Config>,
}

impl Model {
    pub fn open(path: &Path) -> Result<Self, ()> {
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
        let predictor = Predictor::open(path, None).map_err(|_| ())?;
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
            request: None,
        })
    }
}

fn display_word(word: String) -> String {
    match word.as_str() {
        "i" | "i'm" | "i'll" | "i'd" | "i've" => "I".to_owned() + &word[1..],
        _ => word,
    }
}
impl Predict for Model {
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        // Child inference starts only after the parent has contained this worker
        // and sent a query, never during speculative statistical loading.
        if let Some(config) = self.config.take() {
            self.neural = switchify_prediction_neural::Refiner::new(config).ok();
        }
        let options = Options {
            limit: 5,
            min_chars: 0,
            unigram_only: false,
        };
        if let Some(neural) = &mut self.neural {
            if let Ok(immediate) = neural.submit(&self.predictor, before, prefix, options, 0) {
                self.request = immediate
                    .refinement_requested
                    .then_some(immediate.request_id);
                return Ok(Prediction {
                    words: immediate.words.into_iter().map(display_word).collect(),
                });
            }
        }
        self.request = None;
        Ok(Prediction {
            words: self
                .predictor
                .predict(before, prefix, options)
                .into_iter()
                .map(|s| display_word(s.word))
                .collect(),
        })
    }
    fn poll(&mut self) -> Option<Prediction> {
        let result = self.neural.as_mut()?.poll()?;
        if self.request != Some(result.request_id) {
            return None;
        }
        self.request = None;
        Some(Prediction {
            words: result.words.into_iter().map(display_word).collect(),
        })
    }
    fn pending(&self) -> bool {
        self.request.is_some()
            && self.neural.as_ref().is_some_and(|n| {
                matches!(
                    n.status(),
                    switchify_prediction_neural::Status::Ready
                        | switchify_prediction_neural::Status::Loading
                )
            })
    }
    fn reset(&mut self) {
        self.request = None;
        if let Some(n) = &mut self.neural {
            n.reset();
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
            request: None,
        };
        model.config = Some(switchify_prediction_neural::Config {
            bundle: dir.0.join("missing"),
            portable_worker: dir.0.join("missing-worker"),
            accelerated_worker: None,
            threads: 4,
        });
        assert_eq!(model.predict("I need ", "h").unwrap().words[0], "help");
        assert!(!model.pending());
        assert!(model.config.is_none());
        assert!(model.poll().is_none());
        assert_eq!(model.predict("I drink ", "").unwrap().words[0], "water");
        assert!(model.predict("", "zyzzy").unwrap().words.is_empty());
        assert_eq!(
            model.predict("", "cafe\u{301}").unwrap().words,
            vec!["café"]
        );
        assert_eq!(model.predict("", "can’").unwrap().words, vec!["can't"]);
        assert_eq!(model.predict("", "i’").unwrap().words, vec!["I'm"]);
        assert_eq!(model.predict("", "h").unwrap().words.len(), 5);
        assert_eq!(
            model.predict("I need.", "h").unwrap(),
            model.predict("", "h").unwrap()
        );
        drop(model);
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        // A structurally valid custom database is not the pinned production model.
        assert!(Model::open(&path).is_err());
        fs::write(&path, "corrupt").unwrap();
        assert!(Model::open(&path).is_err());
        assert!(Model::open(&dir.0.join("missing")).is_err());
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
                    .join("resources/prediction-model/english.sqlite")
            });
        let before = rss_bytes();
        let start = Instant::now();
        let mut model = Model::open(&path).unwrap();
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
            assert!(!suggestions.words.is_empty());
            if i >= 20 {
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        samples.sort_by(f64::total_cmp);
        assert_eq!(model.predict("I need ", "he").unwrap().words[0], "help");
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
