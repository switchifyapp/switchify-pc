use super::worker::{receive, send};
use crate::prediction::model::{Predict, Prediction};
use serde::Deserialize;
use std::{
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Local native trial adapter. Launch argv is explicit JSON, never a shell command.
pub(in crate::prediction) struct TextSlinger {
    pub(in crate::prediction) child: Child,
    pub(in crate::prediction) input: Option<ChildStdin>,
    replies: mpsc::Receiver<Result<serde_json::Value, ()>>,
    reader: Option<JoinHandle<()>>,
    healthy: bool,
    sequence: u64,
}

impl TextSlinger {
    #[cfg(test)]
    pub(in crate::prediction) fn open() -> Self {
        Self::launch(&Self::argv())
            .expect("backend startup failed; check the model and launcher paths")
    }

    #[cfg(test)]
    pub(in crate::prediction) fn argv() -> Vec<String> {
        serde_json::from_str(
            &std::env::var("SWITCHIFY_TEXTSLINGER_COMMAND")
                .expect("set SWITCHIFY_TEXTSLINGER_COMMAND to a JSON argv array"),
        )
        .expect("invalid launcher argv")
    }

    pub(in crate::prediction) fn launch(argv: &[String]) -> Result<Self, ()> {
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
        let mut output = child.stdout.take().unwrap();
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
        let ready = adapter.reply(Duration::from_secs(90))?;
        if ready != serde_json::json!({"ready": true}) {
            return Err(());
        }
        Ok(adapter)
    }

    pub(in crate::prediction) fn reply(
        &mut self,
        deadline: Duration,
    ) -> Result<serde_json::Value, ()> {
        self.replies.recv_timeout(deadline).map_err(|_| ())?
    }

    fn query(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()> {
        if !self.healthy {
            return Err(());
        }
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        send(
            self.input.as_mut().ok_or(())?,
            &serde_json::json!({"id": self.sequence, "before": before, "prefix": prefix}),
        )?;
        let value = self.reply(Duration::from_millis(1200))?;
        prediction_reply(value, self.sequence, prefix)
    }
}

pub(in crate::prediction) fn prediction_reply(
    value: serde_json::Value,
    sequence: u64,
    prefix: &str,
) -> Result<Prediction, ()> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reply {
        id: u64,
        words: Vec<String>,
    }
    let reply: Reply = serde_json::from_value(value).map_err(|_| ())?;
    if reply.id != sequence || reply.words.len() > 5 {
        return Err(());
    }
    let prefix = prefix.to_lowercase();
    let mut words = reply.words.into_iter().filter(|word| {
        !word.is_empty()
            && word.len() <= 128
            && word.chars().all(|c| c.is_ascii_lowercase() || c == '\'')
            && word.starts_with(&prefix)
    });
    Ok(Prediction {
        words: std::array::from_fn(|_| words.next()),
        ..Prediction::default()
    })
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
        if self.healthy {
            if let Some(mut input) = self.input.take() {
                let _ = send(&mut input, &serde_json::json!({"shutdown": true}));
            }
            let until = Instant::now() + Duration::from_secs(2);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        // A failed query has already consumed up to 1.2 seconds of the
        // parent's two-second deadline. Do not wait for a busy helper to
        // accept shutdown before reporting Unavailable to the keyboard.
        drop(self.input.take());
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
