//! Word suggestions from TextSlinger, run by the bundled runtime.
//!
//! The worker starts the runtime's Python with `bridge.py` and talks to it
//! over private framed pipes (see `resources/textslinger/bridge.py`). The
//! child inherits the worker's containment: its Windows kill-on-close job, or
//! its macOS process group, so it never outlives the worker.
use super::worker::{receive, send};
use crate::prediction::{
    model::{Predict, Prediction},
    nearby::{correctable, KeyPosition},
};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Loading imports Python and maps the model. A cold disk after a restart, or
/// an antivirus scan of the runtime's files, can make the first start slow.
const STARTUP: Duration = Duration::from_secs(90);
/// Within the worker's slow-call allowance and the parent's reply deadline.
const REPLY: Duration = Duration::from_millis(1200);

/// The command that starts the bridge with the runtime in `runtime`.
pub(in crate::prediction) fn command(runtime: &Path) -> Result<Vec<String>, ()> {
    #[derive(Deserialize)]
    struct Manifest {
        format_version: u32,
        python: String,
    }
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(runtime.join("manifest.json")).map_err(|_| ())?)
            .map_err(|_| ())?;
    if manifest.format_version != 1 {
        return Err(());
    }
    let python = runtime.join(&manifest.python);
    let bridge: PathBuf = runtime.parent().ok_or(())?.join("bridge.py");
    // The interpreter must be inside the runtime, never a path the manifest
    // points elsewhere.
    let inside = python
        .canonicalize()
        .ok()
        .zip(runtime.canonicalize().ok())
        .is_some_and(|(python, runtime)| python.starts_with(runtime));
    if !inside || !bridge.is_file() {
        return Err(());
    }
    let text = |path: &Path| path.to_str().map(str::to_owned).ok_or(());
    Ok(vec![
        text(&python)?,
        // Isolated: no user site-packages or PYTHON* variables. No bytecode
        // is written, since the runtime may be in a read-only folder.
        "-I".into(),
        "-B".into(),
        text(&bridge)?,
        "--runtime".into(),
        text(runtime)?,
    ])
}

pub(in crate::prediction) struct TextSlinger {
    pub(in crate::prediction) child: Child,
    pub(in crate::prediction) input: Option<ChildStdin>,
    replies: mpsc::Receiver<Result<serde_json::Value, ()>>,
    reader: Option<JoinHandle<()>>,
    healthy: bool,
    sequence: u64,
}

impl TextSlinger {
    /// Starts the bridge and waits until it has loaded the model.
    pub(in crate::prediction) fn launch(argv: &[String]) -> Result<Self, ()> {
        Self::launch_within(argv, STARTUP)
    }

    pub(in crate::prediction) fn launch_within(
        argv: &[String],
        startup: Duration,
    ) -> Result<Self, ()> {
        let mut command = Command::new(argv.first().filter(|arg| !arg.is_empty()).ok_or(())?);
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
            .map_err(|_| ())?;
        let input = child.stdin.take();
        let Some(mut output) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(());
        };
        let (tx, replies) = mpsc::sync_channel(1);
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
            sequence: 0,
        };
        match adapter.reply(startup) {
            Ok(ready) if ready == serde_json::json!({"ready": true}) => Ok(adapter),
            _ => {
                adapter.healthy = false;
                Err(())
            }
        }
    }

    pub(in crate::prediction) fn reply(
        &mut self,
        deadline: Duration,
    ) -> Result<serde_json::Value, ()> {
        self.replies.recv_timeout(deadline).map_err(|_| ())?
    }

    fn query(
        &mut self,
        before: &str,
        prefix: &str,
        keys: &[KeyPosition],
    ) -> Result<Prediction, ()> {
        if !self.healthy {
            return Err(());
        }
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        let mut request =
            serde_json::json!({"id": self.sequence, "before": before, "prefix": prefix});
        // Keys only matter when the bridge may correct this prefix.
        let corrections = !keys.is_empty() && correctable(prefix);
        if corrections {
            request["keys"] = serde_json::to_value(keys).map_err(|_| ())?;
        }
        send(self.input.as_mut().ok_or(())?, &request)?;
        let value = self.reply(REPLY)?;
        prediction_reply(value, self.sequence, prefix, corrections)
    }
}

/// Checks a reply against the request it answers. Words that are not plain
/// lowercase completions of the prefix are dropped, unless `corrections`
/// allows a plain lowercase word that replaces it; anything else in the
/// reply makes it invalid.
pub(in crate::prediction) fn prediction_reply(
    value: serde_json::Value,
    sequence: u64,
    prefix: &str,
    corrections: bool,
) -> Result<Prediction, ()> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reply {
        id: u64,
        words: Vec<String>,
    }
    let reply: Reply = serde_json::from_value(value).map_err(|_| ())?;
    if reply.id != sequence || reply.words.len() > super::SLOTS {
        return Err(());
    }
    let prefix = prefix.to_lowercase();
    let mut words = reply.words.into_iter().filter(|word| {
        !word.is_empty()
            && word.len() <= 128
            && word.chars().all(|c| c.is_ascii_lowercase() || c == '\'')
            && (corrections || word.starts_with(&prefix))
    });
    Ok(Prediction {
        words: std::array::from_fn(|_| words.next()),
    })
}

impl Predict for TextSlinger {
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        self.predict_near(before, prefix, &[])
    }
    fn predict_near(
        &mut self,
        before: &str,
        prefix: &str,
        keys: &[KeyPosition],
    ) -> Result<Prediction, ()> {
        let result = self.query(before, prefix, keys);
        // Never let a late response answer another request after a timeout.
        self.healthy &= result.is_ok();
        result
    }
}

impl Drop for TextSlinger {
    fn drop(&mut self) {
        if self.healthy {
            if let Some(mut input) = self.input.take() {
                let _ = send(&mut input, &serde_json::json!({"shutdown": true}));
            }
            let until = Instant::now() + Duration::from_secs(2);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        // A failed query has already used up to 1.2 seconds of the parent's
        // reply deadline, so an unhealthy helper is not given time to stop.
        drop(self.input.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Drop the receiving end first: a reader blocked delivering a reply
        // nobody will read then fails its send and exits instead of hanging.
        let (_, closed) = mpsc::sync_channel(0);
        drop(std::mem::replace(&mut self.replies, closed));
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
