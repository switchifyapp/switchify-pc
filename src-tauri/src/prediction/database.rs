use super::{
    context::Context,
    model::{Predict, Prediction},
};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

/// Repeated stalls mark the model unavailable before the parent reply deadline.
const SLOW_CALL: Duration = Duration::from_millis(1500);
/// Slow calls in a row that mark the model unavailable. One stall, such as
/// a busy disk or a waking laptop, is not a model that cannot keep up.
const SLOW_CALLS: u8 = 3;
/// Failed calls in a row after which a model that can be started again is
/// left unavailable. Each failure before that starts it again, so one missed
/// reply does not need Retry predictions.
const FAILURES: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Status {
    Loading,
    Ready,
    Unavailable,
}

type Loaded = Result<Box<dyn Predict>, ()>;
enum State {
    /// Not started yet: the worker starts the model once the parent has
    /// contained it, on its first request.
    Startup(Vec<String>),
    Loading(mpsc::Receiver<Loaded>),
    Ready(Box<dyn Predict>),
    Unavailable,
}

/// Loading is asynchronous so every keyboard edit remains usable. There is no
/// secondary suggestion source when the model is unavailable.
pub struct Database {
    state: State,
    slow_call: Duration,
    slow_calls: u8,
    /// How to start the model again after a failed call, when it can be.
    relaunch: Option<Vec<String>>,
    failures: u8,
}
impl Database {
    fn with_state(state: State) -> Self {
        Self {
            state,
            slow_call: SLOW_CALL,
            slow_calls: 0,
            relaunch: None,
            failures: 0,
        }
    }
    /// The TextSlinger runtime in `runtime`. Nothing starts until the first
    /// request.
    pub fn open(runtime: &Path) -> Self {
        match super::textslinger::command(runtime) {
            Ok(argv) => Self::textslinger(argv),
            Err(()) => Self::with_state(State::Unavailable),
        }
    }
    /// The bridge started with `argv`, once the first request arrives. On
    /// Windows that is after the parent has put the worker in its kill job,
    /// so the runtime cannot outlive it.
    pub fn textslinger(argv: Vec<String>) -> Self {
        Self {
            relaunch: Some(argv.clone()),
            ..Self::with_state(State::Startup(argv))
        }
    }
    fn load(loader: impl FnOnce() -> Loaded + Send + 'static) -> State {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = tx.send(loader());
        });
        State::Loading(rx)
    }
    pub fn status(&mut self) -> Status {
        if let State::Startup(argv) = &mut self.state {
            let argv = std::mem::take(argv);
            self.state = Self::load(move || {
                super::textslinger::TextSlinger::launch(&argv)
                    .map(|model| Box::new(model) as Box<dyn Predict>)
            });
        }
        if let State::Loading(rx) = &self.state {
            self.state = match rx.try_recv() {
                Ok(Ok(model)) => State::Ready(model),
                Err(mpsc::TryRecvError::Empty) => return Status::Loading,
                _ => State::Unavailable,
            };
        }
        match self.state {
            State::Startup(_) | State::Loading(_) => Status::Loading,
            State::Ready(_) => Status::Ready,
            State::Unavailable => Status::Unavailable,
        }
    }
    pub fn poll(&mut self) -> Option<Prediction> {
        if let State::Ready(model) = &mut self.state {
            model.poll()
        } else {
            None
        }
    }
    pub fn pending(&self) -> bool {
        matches!(&self.state, State::Ready(model) if model.pending())
    }
    pub fn reset(&mut self) {
        if let State::Ready(model) = &mut self.state {
            model.reset();
        }
    }
    pub fn predict(&mut self, context: &Context) -> (Status, Prediction) {
        let status = self.status();
        if status != Status::Ready || context.partial {
            return (status, Prediction::default());
        }
        let State::Ready(model) = &mut self.state else {
            unreachable!()
        };
        let start = Instant::now();
        let result = model.predict(&context.before, &context.prefix);
        self.slow_calls = if start.elapsed() > self.slow_call {
            self.slow_calls + 1
        } else {
            0
        };
        let Some(prediction) = result.ok().filter(|_| self.slow_calls < SLOW_CALLS) else {
            self.failures = self.failures.saturating_add(1);
            self.slow_calls = 0;
            // The failed model is dropped, which stops it, and a fresh one
            // starts loading, unless it has failed too often in a row.
            self.state = match &self.relaunch {
                Some(argv) if self.failures < FAILURES => State::Startup(argv.clone()),
                _ => State::Unavailable,
            };
            let status = self.status();
            return (status, Prediction::default());
        };
        self.failures = 0;
        (Status::Ready, prediction)
    }
    #[cfg(test)]
    pub fn pending_fixture() -> (Self, mpsc::SyncSender<Loaded>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (Self::with_state(State::Loading(rx)), tx)
    }

    #[cfg(test)]
    pub fn with_predictor(model: Box<dyn Predict>) -> Self {
        Self::with_state(State::Ready(model))
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self::with_state(State::Ready(Box::new(FakeModel)))
    }
}

/// Words that exercise the worker's casing, accent and apostrophe handling,
/// matched in canonical form and without regard to case.
#[cfg(test)]
struct FakeModel;
#[cfg(test)]
impl Predict for FakeModel {
    fn predict(&mut self, _before: &str, prefix: &str) -> Result<Prediction, ()> {
        use unicode_normalization::UnicodeNormalization;
        let fold = |text: &str| -> String {
            text.nfc()
                .flat_map(char::to_lowercase)
                .map(|c| if c == '\u{2019}' { '\'' } else { c })
                .collect()
        };
        let prefix = fold(prefix);
        let words: Vec<String> = [
            "water", "waffle", "walk", "WhatsApp", "café", "can't", "I'm",
        ]
        .into_iter()
        .filter(|w| fold(w).starts_with(&prefix))
        .map(str::to_owned)
        .collect();
        Ok(Prediction {
            words: super::model::slots(words),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Result<Vec<&'static str>, ()>, Duration);
    impl Predict for Fake {
        fn predict(&mut self, _before: &str, _prefix: &str) -> Result<Prediction, ()> {
            std::thread::sleep(self.1);
            self.0.clone().map(|w| Prediction {
                words: super::super::model::slots(w),
            })
        }
    }
    fn context() -> Context {
        super::super::context::extract("I would like wa", false)
    }
    #[test]
    fn loading_failure_and_missing_runtime_have_no_fallback() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut db = Database::with_state(State::Loading(rx));
        assert_eq!(
            db.predict(&context()),
            (Status::Loading, Prediction::default())
        );
        tx.send(Err(())).unwrap();
        assert_eq!(
            db.predict(&context()),
            (Status::Unavailable, Prediction::default())
        );
        let missing =
            std::env::temp_dir().join(format!("switchify-missing-runtime-{}", std::process::id()));
        let mut db = Database::open(&missing);
        assert_eq!(
            db.predict(&context()),
            (Status::Unavailable, Prediction::default())
        );
    }
    #[test]
    fn a_runtime_that_cannot_start_becomes_unavailable() {
        let mut db = Database::textslinger(vec![std::env::temp_dir()
            .join(format!("switchify-no-python-{}", std::process::id()))
            .to_string_lossy()
            .into_owned()]);
        let end = Instant::now() + Duration::from_secs(5);
        while db.status() == Status::Loading && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(db.status(), Status::Unavailable);
    }
    #[test]
    fn only_repeated_slow_calls_make_the_model_unavailable() {
        // Wide margins, so a busy machine cannot make a prompt call look slow.
        let late = || State::Ready(Box::new(Fake(Ok(vec!["late"]), Duration::from_millis(250))));
        let prompt = || State::Ready(Box::new(Fake(Ok(vec!["prompt"]), Duration::ZERO)));
        let mut db = Database::fixture();
        db.slow_call = Duration::from_millis(200);
        // A slow call still answers, and a prompt one forgives it.
        {
            db.state = late();
            for _ in 1..SLOW_CALLS {
                let (status, prediction) = db.predict(&context());
                assert_eq!(
                    (status, prediction.words),
                    (Status::Ready, super::super::model::slots(["late"]))
                );
            }
            db.state = prompt();
            assert_eq!(db.predict(&context()).0, Status::Ready);
        }
        db.state = late();
        for _ in 1..SLOW_CALLS {
            assert_eq!(db.predict(&context()).0, Status::Ready);
        }
        assert_eq!(
            db.predict(&context()),
            (Status::Unavailable, Prediction::default())
        );
        assert_eq!(
            db.predict(&context()),
            (Status::Unavailable, Prediction::default())
        );
        // A model error is not forgiven, however fast.
        let mut db = Database::fixture();
        db.state = State::Ready(Box::new(Fake(Err(()), Duration::ZERO)));
        assert_eq!(db.predict(&context()).0, Status::Unavailable);
    }
}
