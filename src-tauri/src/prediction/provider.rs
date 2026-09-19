use super::{context::Context, database::Database, worker};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

pub const ARG: &str = "--switchify-native-prediction-worker";
const BUDGET: Duration = Duration::from_millis(600);

pub trait Provider {
    fn predict(&mut self, context: &Context, before: &str) -> Result<Vec<String>, ()>;
}
impl Provider for Database {
    fn predict(&mut self, context: &Context, _: &str) -> Result<Vec<String>, ()> {
        Database::predict(self, context)
    }
}
pub struct WithFallback<N, F> {
    pub native: N,
    pub fallback: F,
}
impl<N: Provider, F: Provider> Provider for WithFallback<N, F> {
    fn predict(&mut self, context: &Context, before: &str) -> Result<Vec<String>, ()> {
        let words = usable(
            self.native.predict(context, before).unwrap_or_default(),
            &context.prefix,
        );
        if !words.is_empty() {
            return Ok(words);
        }
        self.fallback
            .predict(context, before)
            .map(|words| usable(words, &context.prefix))
    }
}
pub fn usable(words: Vec<String>, prefix: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    words
        .into_iter()
        .take(64)
        .filter_map(|word| {
            if word.contains(['\n', '\r']) {
                return None;
            }
            let word = word.trim();
            let offset = word
                .char_indices()
                .nth(prefix.chars().count())
                .map_or(word.len(), |(i, _)| i);
            let folded = word.to_lowercase();
            (!word.is_empty()
                && word.chars().count() <= 48
                && word.unicode_words().next() == Some(word)
                && word[..offset].to_lowercase() == prefix.to_lowercase()
                && seen.insert(folded))
            .then(|| word.to_owned())
        })
        .take(5)
        .collect()
}
#[derive(Serialize, Deserialize)]
struct Input {
    before: String,
    prefix: String,
    words: Vec<String>,
}
impl Input {
    fn valid(&self) -> bool {
        self.before.chars().count() <= 512
            && self.prefix.chars().count() <= 512
            && self.before.ends_with(&self.prefix)
            && self.words.len() <= 3
            && self.words.iter().all(|w| w.chars().count() <= 512)
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub struct Native;
impl Provider for Native {
    fn predict(&mut self, context: &Context, before: &str) -> Result<Vec<String>, ()> {
        let input = Input {
            before: before.to_owned(),
            prefix: context.prefix.clone(),
            words: context.words.clone(),
        };
        let mut command = Command::new(std::env::current_exe().map_err(|_| ())?);
        command.arg(ARG);
        run(command, input, BUDGET)
    }
}
fn run(mut command: Command, input: Input, budget: Duration) -> Result<Vec<String>, ()> {
    if !input.valid() {
        return Err(());
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = OwnedChild(command.spawn().map_err(|_| ())?);
    #[cfg(target_os = "windows")]
    let _job = super::Job::contain(&child.0)?;
    let mut input_pipe = child.0.stdin.take().ok_or(())?;
    let mut output = child.0.stdout.take().ok_or(())?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let result = worker::send(&mut input_pipe, &input)
            .and_then(|()| worker::receive::<Vec<String>>(&mut output));
        let _ = sender.send(result);
    });
    let result = receiver
        .recv_timeout(budget)
        .map_err(|_| ())
        .and_then(|r| r);
    drop(child);
    let _ = reader.join();
    result
}
pub fn probe_from_args() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--switchify-native-prediction-probe") {
        return false;
    }
    let _ = (|| -> Result<(), ()> {
        let path = std::env::args_os().nth(2).ok_or(())?;
        let database = Database::open(std::path::Path::new(&path))?;
        let mode = std::env::args()
            .nth(3)
            .unwrap_or_else(|| "unspecified".into());
        let mut native = Native;
        for repetition in 0..5 {
            for before in [
                "hel",
                "wat",
                "I want some wa",
                "Thank you ",
                "How are ",
                "I would like ",
            ] {
                let context = super::context::extract(before, false);
                let start = std::time::Instant::now();
                let result = native.predict(&context, before);
                let native_ms = start.elapsed().as_secs_f64() * 1000.0;
                let error = result.is_err();
                let words = usable(result.unwrap_or_default(), &context.prefix);
                let start = std::time::Instant::now();
                let fallback = database.predict(&context)?;
                let database_ms = start.elapsed().as_secs_f64() * 1000.0;
                println!(
                    "{}",
                    serde_json::json!({"mode":mode,"repetition":repetition,"text":before,"native":words,"database":fallback,"native_error":error,"fallback_used":words.is_empty(),"native_ms":native_ms,"database_ms":database_ms})
                );
            }
        }
        Ok(())
    })();
    true
}

pub fn run_from_args() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(ARG)) {
        return false;
    }
    worker::watch_parent();
    let _ = (|| -> Result<(), ()> {
        let input: Input = worker::receive(&mut std::io::stdin().lock())?;
        if !input.valid() {
            return Err(());
        }
        let words = platform(&input)?;
        worker::send(&mut std::io::stdout().lock(), &words)
    })();
    true
}
#[cfg(target_os = "macos")]
fn platform(input: &Input) -> Result<Vec<String>, ()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSSpellChecker};
    use objc2_foundation::{NSRange, NSString};
    let marker = MainThreadMarker::new().ok_or(())?;
    NSApplication::sharedApplication(marker)
        .setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let checker = NSSpellChecker::sharedSpellChecker();
    let available = checker.availableLanguages();
    let language = if available.iter().any(|s| s.to_string() == "en_GB") {
        "en_GB"
    } else {
        "en"
    };
    let text = NSString::from_str(&input.before);
    let prefix_length = input.prefix.encode_utf16().count();
    let range = NSRange::new(
        input.before.encode_utf16().count() - prefix_length,
        prefix_length,
    );
    let tag = NSSpellChecker::uniqueSpellDocumentTag();
    let result = checker.completionsForPartialWordRange_inString_language_inSpellDocumentWithTag(
        range,
        &text,
        Some(&NSString::from_str(language)),
        tag,
    );
    let words = result
        .map(|words| words.iter().take(64).map(|s| s.to_string()).collect())
        .unwrap_or_default();
    checker.closeSpellDocumentWithTag(tag);
    Ok(usable(words, &input.prefix))
}
#[cfg(target_os = "windows")]
fn platform(input: &Input) -> Result<Vec<String>, ()> {
    use windows::{
        core::HSTRING,
        Data::Text::{TextPredictionGenerator, TextPredictionOptions},
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
    };
    use windows_collections::IIterable;
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED).map_err(|_| ())?;
    }
    locales(&["en-GB", "en-US"], &input.prefix, |language| {
        let generator =
            TextPredictionGenerator::Create(&HSTRING::from(language)).map_err(|_| ())?;
        let prior: IIterable<HSTRING> = input
            .words
            .iter()
            .map(HSTRING::from)
            .collect::<Vec<_>>()
            .into();
        let result = if input.prefix.is_empty() {
            generator.GetNextWordCandidatesAsync(10, &prior)
        } else {
            generator.GetCandidatesWithParametersAsync(
                &HSTRING::from(&input.prefix),
                10,
                TextPredictionOptions::Predictions,
                &prior,
            )
        }
        .and_then(|op| op.join())
        .map_err(|_| ())?;
        (0..result.Size().map_err(|_| ())?.min(64))
            .map(|i| result.GetAt(i).map(|s| s.to_string()).map_err(|_| ()))
            .collect()
    })
}
#[cfg(any(target_os = "windows", test))]
fn locales(
    mut languages: &[&str],
    prefix: &str,
    mut predict: impl FnMut(&str) -> Result<Vec<String>, ()>,
) -> Result<Vec<String>, ()> {
    while let Some((language, rest)) = languages.split_first() {
        let words = usable(predict(language).unwrap_or_default(), prefix);
        if !words.is_empty() {
            return Ok(words);
        }
        languages = rest;
    }
    Ok(Vec::new())
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform(_: &Input) -> Result<Vec<String>, ()> {
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        result: Result<Vec<String>, ()>,
        calls: usize,
    }
    impl Provider for Fake {
        fn predict(&mut self, _: &Context, _: &str) -> Result<Vec<String>, ()> {
            self.calls += 1;
            self.result.clone()
        }
    }
    fn fake(words: &[&str]) -> Fake {
        Fake {
            result: Ok(words.iter().map(|s| s.to_string()).collect()),
            calls: 0,
        }
    }
    #[test]
    fn native_ranking_filtering_and_fallback() {
        let mut provider = WithFallback {
            native: fake(&["water", "walk", "water", "wrong", "wa\nter", "water melon"]),
            fallback: fake(&["want"]),
        };
        let context = super::super::context::extract("I want wa", false);
        assert_eq!(
            provider.predict(&context, "I want wa").unwrap(),
            ["water", "walk"]
        );
        assert_eq!(provider.fallback.calls, 0);
        provider.native.result = Err(());
        assert_eq!(provider.predict(&context, "I want wa").unwrap(), ["want"]);
        assert_eq!(provider.fallback.calls, 1);
        provider.native = fake(&["", "\n", "wrong", " "]);
        assert_eq!(provider.predict(&context, "I want wa").unwrap(), ["want"]);
        provider.fallback.result = Err(());
        assert!(provider.predict(&context, "I want wa").is_err());
    }
    #[test]
    fn candidate_bounds_and_unicode_prefixes() {
        assert_eq!(
            usable(
                vec![
                    " Café ".into(),
                    "CAFE".into(),
                    "café".into(),
                    "café\n".into()
                ],
                "Caf"
            ),
            ["Café", "CAFE"]
        );
        assert!(usable(vec!["a".repeat(49), "two words".into(), "a\0".into()], "").is_empty());
        assert_eq!(
            usable(
                vec![
                    "for".into(),
                    "so".into(),
                    "again".into(),
                    "and".into(),
                    "very".into(),
                    "much".into()
                ],
                ""
            )
            .len(),
            5
        );
        assert_eq!(usable(vec!["can't".into()], "can"), ["can't"]);
    }
    #[test]
    fn retry_us_only_when_uk_is_unusable() {
        let mut tried = Vec::new();
        let words = locales(&["en-GB", "en-US"], "wa", |lang| {
            tried.push(lang.to_owned());
            Ok(if lang == "en-GB" {
                vec!["wrong".into()]
            } else {
                vec!["water".into()]
            })
        })
        .unwrap();
        assert_eq!(tried, ["en-GB", "en-US"]);
        assert_eq!(words, ["water"]);
        let mut count = 0;
        assert_eq!(
            locales(&["en-GB", "en-US"], "", |_| {
                count += 1;
                Ok(vec!["for".into()])
            })
            .unwrap(),
            ["for"]
        );
        assert_eq!(count, 1);
    }
    #[test]
    fn hung_native_process_is_stopped_before_fallback_deadline() {
        #[cfg(target_os = "windows")]
        let command = {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"]);
            c
        };
        #[cfg(not(target_os = "windows"))]
        let command = {
            let mut c = Command::new("/bin/sleep");
            c.arg("10");
            c
        };
        let began = std::time::Instant::now();
        let result = run(
            command,
            Input {
                before: "wa".into(),
                prefix: "wa".into(),
                words: Vec::new(),
            },
            Duration::from_millis(100),
        );
        assert!(result.is_err());
        assert!(began.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn invalid_context_never_starts_a_native_helper() {
        let result = run(
            Command::new("nonexistent-probe"),
            Input {
                before: "hello".into(),
                prefix: "wrong".into(),
                words: Vec::new(),
            },
            BUDGET,
        );
        assert!(result.is_err());
    }
}
