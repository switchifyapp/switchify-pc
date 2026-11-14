use super::{
    context::Context,
    model::{Model, Predict, Prediction},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Status {
    Loading,
    Ready,
    Unavailable,
}

type Loaded = Result<Box<dyn Predict>, ()>;
enum State {
    Loading(mpsc::Receiver<Loaded>),
    #[cfg(any(test, feature = "textslinger-trial"))]
    TextSlingerStartup(Vec<String>),
    Ready(Box<dyn Predict>),
    Unavailable,
}

/// Loading is asynchronous so every keyboard edit remains usable. There is no
/// secondary suggestion source when the model is unavailable.
pub struct Database {
    state: State,
    slow_call: Duration,
    slow_calls: u8,
}
impl Database {
    #[cfg(test)]
    pub fn neural_status(&self) -> Option<switchify_prediction_neural::Status> {
        match &self.state {
            State::Ready(model) => model.neural_status(),
            _ => None,
        }
    }
    pub fn open(model: &Path, personal: Option<&Path>) -> Self {
        let model = model.to_path_buf();
        let personal = personal.map(Path::to_path_buf);
        Self::load(move || {
            Model::open(&model, personal.as_deref())
                .map(|model| Box::new(model) as Box<dyn Predict>)
        })
    }
    fn load(loader: impl FnOnce() -> Loaded + Send + 'static) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = tx.send(loader());
        });
        Self {
            state: State::Loading(rx),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        }
    }
    #[cfg(feature = "textslinger-trial")]
    pub fn trial_from_env() -> Option<Self> {
        let command = std::env::var_os("SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND")?;
        let argv = command
            .to_str()
            .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
        Some(
            match argv.filter(|args| {
                !args.is_empty() && args.len() <= 16 && args.iter().all(|arg| arg.len() <= 4096)
            }) {
                Some(argv) => Self::textslinger(argv),
                None => Self {
                    state: State::Unavailable,
                    slow_call: SLOW_CALL,
                    slow_calls: 0,
                },
            },
        )
    }
    #[cfg(any(test, feature = "textslinger-trial"))]
    pub fn textslinger(argv: Vec<String>) -> Self {
        // Defer child creation until the parent sends its first request. On
        // Windows that happens after the worker is contained in its kill job.
        Self {
            state: State::TextSlingerStartup(argv),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        }
    }
    pub fn status(&mut self) -> Status {
        #[cfg(any(test, feature = "textslinger-trial"))]
        if let State::TextSlingerStartup(argv) = &mut self.state {
            let argv = std::mem::take(argv);
            self.state = Self::load(move || {
                super::textslinger::TextSlinger::launch(&argv)
                    .map(|model| Box::new(model) as Box<dyn Predict>)
            })
            .state;
        }
        if let State::Loading(rx) = &self.state {
            self.state = match rx.try_recv() {
                Ok(Ok(model)) => State::Ready(model),
                Err(mpsc::TryRecvError::Empty) => return Status::Loading,
                _ => State::Unavailable,
            };
        }
        match self.state {
            State::Loading(_) => Status::Loading,
            State::Ready(_) => Status::Ready,
            State::Unavailable => Status::Unavailable,
            #[cfg(any(test, feature = "textslinger-trial"))]
            State::TextSlingerStartup(_) => Status::Loading,
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
    /// Text completed before the model is ready is not learned.
    pub fn learn(&mut self, text: &str) {
        if let State::Ready(model) = &mut self.state {
            model.learn(text);
        }
    }
    #[cfg(test)]
    pub fn predict(&mut self, context: &Context) -> (Status, Prediction) {
        self.predict_with_layout(context, None, false)
    }
    pub fn predict_with_layout(
        &mut self,
        context: &Context,
        layout: Option<crate::point_scan::KeyboardLayout>,
        abbreviations: bool,
    ) -> (Status, Prediction) {
        let status = self.status();
        if status != Status::Ready || context.partial {
            return (status, Prediction::default());
        }
        let State::Ready(model) = &mut self.state else {
            unreachable!()
        };
        let start = Instant::now();
        model.set_nearby_layout(layout);
        model.set_abbreviations(abbreviations);
        let result = model.predict(&context.before, &context.prefix);
        self.slow_calls = if start.elapsed() > self.slow_call {
            self.slow_calls + 1
        } else {
            0
        };
        let Some(prediction) = result.ok().filter(|_| self.slow_calls < SLOW_CALLS) else {
            self.state = State::Unavailable;
            return (Status::Unavailable, Prediction::default());
        };
        (Status::Ready, prediction)
    }
    #[cfg(test)]
    pub fn pending_fixture() -> (Self, mpsc::SyncSender<Loaded>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (
            Self {
                state: State::Loading(rx),
                slow_call: SLOW_CALL,
                slow_calls: 0,
            },
            tx,
        )
    }

    #[cfg(test)]
    pub fn with_predictor(model: Box<dyn Predict>) -> Self {
        Self {
            state: State::Ready(model),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        }
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            state: State::Ready(Box::new(FakeModel)),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        }
    }
}

#[cfg(test)]
struct FakeModel;
#[cfg(test)]
impl Predict for FakeModel {
    fn predict(&mut self, _before: &str, prefix: &str) -> Result<Prediction, ()> {
        let words: Vec<String> = [
            "water", "waffle", "walk", "WhatsApp", "café", "can't", "I'm",
        ]
        .into_iter()
        .filter(|w| {
            switchify_prediction::normalize(w).starts_with(&switchify_prediction::normalize(prefix))
        })
        .map(str::to_owned)
        .collect();
        Ok(Prediction {
            corrections: Default::default(),
            abbreviated: Default::default(),
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
                corrections: Default::default(),
                abbreviated: Default::default(),
                words: super::super::model::slots(w),
            })
        }
    }
    fn context() -> Context {
        super::super::context::extract("I would like wa", false)
    }
    #[test]
    fn loading_failure_and_missing_model_have_no_fallback() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut db = Database {
            state: State::Loading(rx),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        };
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
            std::env::temp_dir().join(format!("switchify-missing-model-{}", std::process::id()));
        let mut db = Database::open(&missing, None);
        let end = Instant::now() + Duration::from_secs(5);
        while db.status() == Status::Loading && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            db.predict(&context()),
            (Status::Unavailable, Prediction::default())
        );
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
