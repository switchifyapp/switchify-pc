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
}

pub struct Model(Predictor);
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
        Predictor::open(path, None).map(Self).map_err(|_| ())
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
        Ok(Prediction {
            words: self
                .0
                .predict(
                    before,
                    prefix,
                    Options {
                        limit: 5,
                        min_chars: 0,
                        unigram_only: false,
                    },
                )
                .into_iter()
                .map(|s| display_word(s.word))
                .collect(),
        })
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
        let mut model = Model(Predictor::open(&path, None).unwrap());
        assert_eq!(model.predict("I need ", "h").unwrap().words[0], "help");
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
