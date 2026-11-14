use super::*;
use crate::prediction::model::Predict;
use crate::prediction::textslinger::{prediction_reply, TextSlinger};
use std::time::{Duration, Instant};

#[test]
fn textslinger_failed_load_becomes_unavailable() {
    let mut db = Database::textslinger(vec!["".into()]);
    let until = Instant::now() + Duration::from_secs(2);
    while db.status() == crate::prediction::database::Status::Loading && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        db.status(),
        crate::prediction::database::Status::Unavailable
    );
}

#[test]
#[ignore = "real keyboard backend loading and 35-second idle check; fake input only"]
fn textslinger_keyboard_loading_and_idle() {
    let mut argv = TextSlinger::argv();
    argv.push("--keyboard-trial".into());
    let mut e = engine();
    e.database = Database::textslinger(argv);
    let start = Instant::now();
    let first = e.respond(request(vec![append("I would like a cup of ")], 1, None));
    assert!(matches!(
        first,
        Response::Suggestions {
            status: crate::prediction::database::Status::Loading,
            ..
        }
    ));
    assert!(start.elapsed() < Duration::from_millis(500));
    let until = Instant::now() + Duration::from_secs(95);
    loop {
        let response = e.respond(request(vec![], 1, None));
        if let Response::Suggestions {
            status: crate::prediction::database::Status::Ready,
            batch: Some(batch),
            ..
        } = response
        {
            assert_eq!(batch.words[0].as_deref(), Some("coffee"));
            break;
        }
        assert!(
            Instant::now() < until,
            "keyboard backend did not become ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_secs(35));
    let batch = response_batch(e.respond(request(
        vec![edit(Edit::Reset), append("please open the ")],
        2,
        None,
    )));
    assert_eq!(batch.words[0].as_deref(), Some("door"));
    assert_eq!(e.accept(batch.token, 0), Some((0, "door ".into())));
}

#[test]
#[ignore = "requires a real local TextSlinger model; fake desktop input only"]
fn textslinger_switchify_integration() {
    let startup = Instant::now();
    let adapter = TextSlinger::open();
    println!("TextSlinger ready in {} ms", startup.elapsed().as_millis());
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(adapter));
    let mut timings = Vec::new();
    // Only fixed synthetic fixtures may appear in test output.
    for (text, expected, suffix) in [
        ("I would like a co", "couple", "uple "),
        ("I would like a cup of ", "coffee", "coffee "),
        ("please open the ", "door", "door "),
        ("I would like a cup of CO", "COFFEE", "FFEE "),
    ] {
        let start = Instant::now();
        let batch = response_batch(e.respond(request(
            vec![edit(Edit::Reset), append(text)],
            e.revision + 1,
            None,
        )));
        timings.push(start.elapsed().as_millis());
        assert_eq!(batch.words[0].as_deref(), Some(expected));
        assert!(batch.words[5..].iter().all(Option::is_none));
        let response = e.respond(Request::Accept {
            generation: e.generation,
            token: batch.token,
            revision: e.revision,
            index: 0,
        });
        assert!(
            matches!(response, Response::Insert { backspaces: 0, text: Some(ref text), foreground: Some(1), .. } if text == suffix)
        );
        assert!(e.accept(batch.token, 0).is_none());
    }
    // Backspace must recompute suggestions; a previously shown token cannot
    // insert text after the revision changes.
    let old = response_batch(e.respond(request(
        vec![edit(Edit::Reset), append("I would like a cup of co")],
        e.revision + 1,
        None,
    )));
    let current =
        response_batch(e.respond(request(vec![edit(Edit::Backspace)], e.revision + 1, None)));
    assert_eq!(current.words[0].as_deref(), Some("coffee"));
    assert!(e.accept(old.token, 0).is_none());
    // Losing the foreground invalidates even a valid real-model suggestion.
    e.foreground = || Ok(2);
    assert!(e.accept(current.token, 0).is_none());
    println!("TextSlinger synthetic query round trips in ms: {timings:?}");
}

#[test]
#[ignore = "requires a real local TextSlinger model; fake desktop input only"]
fn textslinger_exit_disables_suggestions() {
    let mut adapter = TextSlinger::open();
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
        matches!(response, Response::Suggestions { status: super::super::super::database::Status::Unavailable, batch: Some(ref batch), .. } if batch.words.iter().all(Option::is_none))
    );
}

#[test]
fn textslinger_reply_ids_and_envelopes_fail_closed() {
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
fn textslinger_invalid_candidates_do_not_break_a_valid_reply() {
    let result = prediction_reply(
        serde_json::json!({"id": 3, "words": ["coffee", "2", "café", "wrong", ""]}),
        3,
        "co",
    )
    .unwrap();
    assert_eq!(result.words, crate::prediction::slots(["coffee"]));
}

#[test]
#[ignore = "requires a real local TextSlinger model; fake desktop input only"]
fn textslinger_word_boundaries_and_unsupported_prefixes() {
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(TextSlinger::open()));
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
        assert_eq!(e.status, crate::prediction::database::Status::Ready);
        if let Some(prefix) = prefix {
            let word = batch.words[0]
                .as_ref()
                .expect("missing boundary completion");
            assert!(word.starts_with(prefix), "{word}");
            assert_eq!(
                e.accept(batch.token, 0),
                Some((0, format!("{} ", &word[prefix.len()..])))
            );
        } else {
            assert!(batch.words.iter().all(Option::is_none));
        }
        // Every fixture, including unsupported input, must leave the session
        // able to produce a real supported completion afterward.
        let next = response_batch(e.respond(request(
            vec![edit(Edit::Reset), append("I would like a cup of ")],
            e.revision + 1,
            None,
        )));
        assert_eq!(next.words[0].as_deref(), Some("coffee"));
    }
}

#[test]
#[ignore = "uses the configured Python launcher to reject a missing model"]
fn textslinger_missing_model_cannot_report_ready() {
    let mut argv = TextSlinger::argv();
    let model = argv.iter().position(|arg| arg == "--model").unwrap() + 1;
    argv[model].push_str(".switchify-spike-missing");
    assert!(TextSlinger::launch(&argv).is_err());
}

#[test]
#[ignore = "uses the configured Python launcher for synthetic fault injection"]
fn textslinger_faults_cannot_answer_another_request() {
    for mode in ["extra", "timeout", "malformed"] {
        let mut argv = TextSlinger::argv();
        let bridge = argv
            .iter_mut()
            .find(|arg| arg.ends_with("textslinger-spike.py"))
            .unwrap();
        *bridge = format!(
            "{}textslinger-spike-test.py",
            bridge.strip_suffix("textslinger-spike.py").unwrap()
        );
        let model = argv.iter().position(|arg| arg == "--model").unwrap();
        argv.truncate(model);
        argv.extend(["--fault".into(), mode.into()]);
        let mut adapter = TextSlinger::launch(&argv).unwrap();
        assert!(adapter.predict("I would like a cup of ", "").is_err());
        let start = Instant::now();
        assert!(adapter.predict("please open the ", "").is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}

#[test]
#[ignore = "uses the configured Python launcher for a stalled helper; fake input only"]
fn textslinger_stall_reports_unavailable_before_parent_deadline() {
    let mut argv = TextSlinger::argv();
    let bridge = argv
        .iter_mut()
        .find(|arg| arg.ends_with("textslinger-spike.py"))
        .unwrap();
    *bridge = format!(
        "{}textslinger-spike-test.py",
        bridge.strip_suffix("textslinger-spike.py").unwrap()
    );
    let model = argv.iter().position(|arg| arg == "--model").unwrap();
    argv.truncate(model);
    argv.extend(["--fault".into(), "stall".into()]);
    let adapter = TextSlinger::launch(&argv).unwrap();
    let mut e = engine();
    e.database = Database::with_predictor(Box::new(adapter));
    let start = Instant::now();
    let response = e.respond(request(vec![append("I would like a cup of ")], 1, None));
    let elapsed = start.elapsed();
    assert!(
        matches!(response, Response::Suggestions { status: crate::prediction::database::Status::Unavailable, batch: Some(ref batch), .. } if batch.words.iter().all(Option::is_none))
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "cleanup exceeded parent deadline: {elapsed:?}"
    );
    assert_eq!(e.buffer, "I would like a cup of ");
}
