//! The worker with the real bundled TextSlinger runtime, and with synthetic
//! faulty peers. Desktop input is always faked; only fixed text is used.
//!
//! The runtime is fetched by `npm run prediction-model`. Where it is missing,
//! as on Linux, the tests that need it report that and pass.
use super::*;
use crate::prediction::database::Status;
use crate::prediction::model::Predict;
use crate::prediction::textslinger::{command, prediction_reply, TextSlinger};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn runtime() -> Option<PathBuf> {
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/textslinger/runtime");
    if runtime.join("manifest.json").is_file() {
        Some(runtime)
    } else {
        eprintln!("No bundled TextSlinger runtime; run npm run prediction-model to test it.");
        None
    }
}

/// The bundled Python running the bridge's synthetic faulty peer.
fn faulty(runtime: &Path, mode: &str) -> Vec<String> {
    let mut argv = command(runtime).unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/textslinger-bridge-test.py");
    // [python, -I, -B, bridge, --runtime, folder] -> [python, -I, -B, script, --fault, mode]
    argv.truncate(3);
    argv.extend([
        script.to_string_lossy().into_owned(),
        "--fault".into(),
        mode.into(),
    ]);
    argv
}

fn ready(e: &mut Engine) -> Batch {
    let until = Instant::now() + Duration::from_secs(95);
    loop {
        if let Response::Suggestions {
            status: Status::Ready,
            batch: Some(batch),
            ..
        } = e.respond(request(vec![], e.revision, None))
        {
            return batch;
        }
        assert!(Instant::now() < until, "TextSlinger did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_runtime_command_needs_a_manifest_an_interpreter_inside_and_the_bridge() {
    let root = std::env::temp_dir().join(format!("switchify-ts-command-{}", std::process::id()));
    let runtime = root.join("runtime");
    std::fs::create_dir_all(runtime.join("bin")).unwrap();
    std::fs::write(runtime.join("bin/python"), b"").unwrap();
    assert!(command(&runtime).is_err(), "no manifest");
    std::fs::write(
        runtime.join("manifest.json"),
        r#"{"format_version":1,"python":"bin/python"}"#,
    )
    .unwrap();
    assert!(command(&runtime).is_err(), "no bridge beside the runtime");
    std::fs::write(root.join("bridge.py"), b"").unwrap();
    let argv = command(&runtime).unwrap();
    assert_eq!(argv[1..3], ["-I".to_owned(), "-B".to_owned()]);
    assert!(argv[3].ends_with("bridge.py"));
    assert_eq!(argv[4], "--runtime");
    for (manifest, why) in [
        (
            r#"{"format_version":2,"python":"bin/python"}"#,
            "unknown format",
        ),
        (
            r#"{"format_version":1,"python":"../outside"}"#,
            "interpreter outside",
        ),
    ] {
        std::fs::write(runtime.join("manifest.json"), manifest).unwrap();
        std::fs::write(root.join("outside"), b"").unwrap();
        assert!(command(&runtime).is_err(), "{why}");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_runtime_that_cannot_start_is_unavailable() {
    let mut db = Database::textslinger(vec!["".into()]);
    let until = Instant::now() + Duration::from_secs(2);
    while db.status() == Status::Loading && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(db.status(), Status::Unavailable);
}

#[test]
fn reply_ids_and_envelopes_fail_closed() {
    for value in [
        serde_json::json!({"id": 1, "words": ["coffee"]}),
        serde_json::json!({"id": 2, "words": ["coffee"], "extra": true}),
        serde_json::json!({"id": 2, "words": ["a", "b", "c", "d", "e", "f"]}),
        serde_json::json!({"id": 2, "words": [null]}),
    ] {
        assert!(prediction_reply(value, 2, "").is_err());
    }
}

#[test]
fn invalid_candidates_do_not_break_a_valid_reply() {
    let result = prediction_reply(
        serde_json::json!({"id": 3, "words": ["coffee", "2", "café", "wrong", ""]}),
        3,
        "co",
    )
    .unwrap();
    assert_eq!(result.words, crate::prediction::slots(["coffee"]));
}

#[test]
fn the_bundled_runtime_loads_suggests_and_inserts() {
    let Some(runtime) = runtime() else { return };
    let mut e = engine();
    e.database = Database::open(&runtime);
    // The model loads in the background; the keyboard is answered at once.
    let start = Instant::now();
    let first = e.respond(request(vec![append("I would like a cup of ")], 1, None));
    assert!(matches!(
        first,
        Response::Suggestions {
            status: Status::Loading,
            ..
        }
    ));
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(ready(&mut e).words[0].as_deref(), Some("coffee"));
    for (text, expected, suffix) in [
        ("I would like a co", "couple", "uple "),
        ("please open the ", "door", "door "),
        ("I would like a cup of CO", "COFFEE", "FFEE "),
    ] {
        let batch = response_batch(e.respond(request(
            vec![edit(Edit::Reset), append(text)],
            e.revision + 1,
            None,
        )));
        assert_eq!(batch.words[0].as_deref(), Some(expected));
        assert!(batch.words.iter().flatten().all(|word| !word.contains(' ')));
        let response = e.respond(Request::Accept {
            generation: e.generation,
            token: batch.token,
            revision: e.revision,
            index: 0,
        });
        assert!(
            matches!(response, Response::Insert { backspaces: 0, text: Some(ref text), foreground: Some(1), .. } if text == suffix)
        );
        assert!(e.accept(batch.token, 0).is_none(), "accepted only once");
    }
    // Backspacing recomputes; an earlier token cannot insert afterwards.
    let old = response_batch(e.respond(request(
        vec![edit(Edit::Reset), append("I would like a cup of co")],
        e.revision + 1,
        None,
    )));
    let current =
        response_batch(e.respond(request(vec![edit(Edit::Backspace)], e.revision + 1, None)));
    assert_eq!(current.words[0].as_deref(), Some("coffee"));
    assert!(e.accept(old.token, 0).is_none());
    // Losing the foreground invalidates even a valid suggestion.
    e.foreground = || Ok(2);
    assert!(e.accept(current.token, 0).is_none());
}

#[test]
fn unsupported_text_leaves_the_model_ready_for_the_next_word() {
    let Some(runtime) = runtime() else { return };
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(
        TextSlinger::launch(&command(&runtime).unwrap()).unwrap(),
    ));
    for (text, prefix) in [
        ("I am 2", None),
        ("café", None),
        ("she is well-kn", Some("kn")),
        ("he said \"he", Some("he")),
        ("a,b", Some("b")),
        ("he said \"", None),
        ("she is well-", None),
        ("hello,", None),
        ("it costs $", None),
        ("look (", None),
    ] {
        let batch = response_batch(e.respond(request(
            vec![edit(Edit::Reset), append(text)],
            e.revision + 1,
            None,
        )));
        assert_eq!(e.status, Status::Ready);
        if let Some(prefix) = prefix {
            let word = batch.words[0].as_ref().expect("a completion");
            assert!(word.starts_with(prefix), "{word}");
            assert_eq!(
                e.accept(batch.token, 0),
                Some((0, format!("{} ", &word[prefix.len()..])))
            );
        } else {
            assert!(batch.words.iter().all(Option::is_none));
        }
        let next = response_batch(e.respond(request(
            vec![edit(Edit::Reset), append("I would like a cup of ")],
            e.revision + 1,
            None,
        )));
        assert_eq!(next.words[0].as_deref(), Some("coffee"));
    }
}

#[test]
fn a_bridge_that_stopped_makes_suggestions_unavailable() {
    let Some(runtime) = runtime() else { return };
    let mut adapter = TextSlinger::launch(&command(&runtime).unwrap()).unwrap();
    assert_eq!(
        adapter.predict("I would like a cup of ", "").unwrap().words[0].as_deref(),
        Some("coffee")
    );
    send(
        adapter.input.as_mut().unwrap(),
        &serde_json::json!({"shutdown": true}),
    )
    .unwrap();
    assert_eq!(
        adapter.reply(Duration::from_secs(2)).unwrap(),
        serde_json::json!({"stopped": true})
    );
    adapter.child.wait().unwrap();
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(adapter));
    let response = e.respond(request(vec![append("I would like a co")], 1, None));
    assert!(
        matches!(response, Response::Suggestions { status: Status::Unavailable, batch: Some(ref batch), .. } if batch.words.iter().all(Option::is_none))
    );
}

#[test]
fn a_runtime_without_its_model_never_reports_ready() {
    let Some(runtime) = runtime() else { return };
    // A copy of the manifest naming a model that does not exist.
    let root = std::env::temp_dir().join(format!("switchify-ts-nomodel-{}", std::process::id()));
    let copy = root.join("runtime");
    std::fs::create_dir_all(&copy).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(runtime.join("manifest.json")).unwrap()).unwrap();
    manifest["model"] = "model/missing.kenlm".into();
    std::fs::write(copy.join("manifest.json"), manifest.to_string()).unwrap();
    let mut argv = command(&runtime).unwrap();
    *argv.last_mut().unwrap() = copy.to_string_lossy().into_owned();
    assert!(TextSlinger::launch_within(&argv, Duration::from_secs(60)).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn faulty_replies_cannot_answer_another_request() {
    let Some(runtime) = runtime() else { return };
    for mode in ["extra", "timeout", "malformed"] {
        let mut adapter = TextSlinger::launch(&faulty(&runtime, mode)).unwrap();
        assert!(
            adapter.predict("I would like a cup of ", "").is_err(),
            "{mode}"
        );
        let start = Instant::now();
        assert!(adapter.predict("please open the ", "").is_err(), "{mode}");
        assert!(start.elapsed() < Duration::from_secs(1), "{mode}");
    }
}

#[test]
fn a_stalled_bridge_is_unavailable_before_the_parent_deadline() {
    let Some(runtime) = runtime() else { return };
    let adapter = TextSlinger::launch(&faulty(&runtime, "stall")).unwrap();
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(adapter));
    let start = Instant::now();
    let response = e.respond(request(vec![append("I would like a cup of ")], 1, None));
    let elapsed = start.elapsed();
    assert!(
        matches!(response, Response::Suggestions { status: Status::Unavailable, batch: Some(ref batch), .. } if batch.words.iter().all(Option::is_none))
    );
    assert!(elapsed < Duration::from_secs(2), "cleanup took {elapsed:?}");
}
