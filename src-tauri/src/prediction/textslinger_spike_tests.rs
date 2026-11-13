use super::*;
use crate::prediction::model::{Predict, Prediction};
use std::{
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Test-only adapter. Launch argv is explicit JSON, never a shell command.
struct TextSlinger {
    child: Child,
    input: Option<ChildStdin>,
    replies: mpsc::Receiver<Result<serde_json::Value, ()>>,
    reader: Option<JoinHandle<()>>,
    healthy: bool,
}

impl TextSlinger {
    fn open() -> Self {
        let argv: Vec<String> = serde_json::from_str(
            &std::env::var("SWITCHIFY_TEXTSLINGER_COMMAND")
                .expect("set SWITCHIFY_TEXTSLINGER_COMMAND to a JSON argv array"),
        )
        .expect("invalid launcher argv");
        let mut command = Command::new(argv.first().expect("empty launcher argv"));
        command.args(&argv[1..]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("could not launch TextSlinger");
        let input = child.stdin.take();
        let mut output = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        let reader = std::thread::spawn(move || loop {
            let result = receive::<serde_json::Value>(&mut output);
            let failed = result.is_err();
            if tx.send(result).is_err() || failed {
                break;
            }
        });
        let mut adapter = Self {
            child,
            input,
            replies,
            reader: Some(reader),
            healthy: true,
        };
        let ready = adapter
            .reply(Duration::from_secs(90))
            .expect("backend startup failed");
        assert_eq!(ready, serde_json::json!({"ready": true}));
        adapter
    }

    fn reply(&mut self, deadline: Duration) -> Result<serde_json::Value, ()> {
        self.replies.recv_timeout(deadline).map_err(|_| ())?
    }

    fn query(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        if !self.healthy {
            return Err(());
        }
        send(
            self.input.as_mut().ok_or(())?,
            &serde_json::json!({"before": before, "prefix": prefix}),
        )?;
        let value = self.reply(Duration::from_millis(1200))?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            words: Vec<String>,
        }
        let reply: Reply = serde_json::from_value(value).map_err(|_| ())?;
        if reply.words.len() > 5
            || reply.words.iter().any(|word| {
                word.is_empty()
                    || word.len() > 128
                    || !word.chars().all(|c| c.is_ascii_lowercase() || c == '\'')
                    || !word.starts_with(&prefix.to_lowercase())
            })
        {
            return Err(());
        }
        Ok(Prediction {
            words: crate::prediction::slots(reply.words),
            ..Prediction::default()
        })
    }
}

impl Predict for TextSlinger {
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        let result = self.query(before, prefix);
        // Never let a late response answer another request after a timeout.
        self.healthy &= result.is_ok();
        result
    }
}

impl Drop for TextSlinger {
    fn drop(&mut self) {
        if let Some(mut input) = self.input.take() {
            let _ = send(&mut input, &serde_json::json!({"shutdown": true}));
        }
        let until = Instant::now() + Duration::from_secs(2);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        // EOF normally joins the reader immediately. A WSL process surviving
        // launcher termination is a documented production cleanup gap.
        if self
            .reader
            .as_ref()
            .is_some_and(|reader| reader.is_finished())
        {
            let _ = self.reader.take().unwrap().join();
        }
    }
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
