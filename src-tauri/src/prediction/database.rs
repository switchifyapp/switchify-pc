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

enum State {
    Loading(mpsc::Receiver<Result<Model, ()>>),
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
    pub fn open(model: &Path) -> Self {
        let model = model.to_path_buf();
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = tx.send(Model::open(&model));
        });
        Self {
            state: State::Loading(rx),
            slow_call: SLOW_CALL,
            slow_calls: 0,
        }
    }
    pub fn status(&mut self) -> Status {
        if let State::Loading(rx) = &self.state {
            self.state = match rx.try_recv() {
                Ok(Ok(model)) => State::Ready(Box::new(model)),
                Err(mpsc::TryRecvError::Empty) => return Status::Loading,
                _ => State::Unavailable,
            };
        }
        match self.state {
            State::Loading(_) => Status::Loading,
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
        let Some(mut prediction) = result.ok().filter(|_| self.slow_calls < SLOW_CALLS) else {
            self.state = State::Unavailable;
            return (Status::Unavailable, Prediction::default());
        };
        prediction.words.truncate(5);
        (Status::Ready, prediction)
    }
    #[cfg(test)]
    pub fn pending_fixture() -> (Self, mpsc::SyncSender<Result<Model, ()>>) {
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
        Ok(Prediction { words })
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
                words: w.iter().map(|w| (*w).to_owned()).collect(),
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
        let mut db = Database::open(&missing);
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
    fn model_only_limit_failure_and_slow_call() {
        let mut db = Database::fixture();
        db.state = State::Ready(Box::new(Fake(
            Ok(vec!["one", "two", "three", "four", "five", "six"]),
            Duration::ZERO,
        )));
        let prediction = db.predict(&context()).1;
        assert_eq!(prediction.words.len(), 5);
        db.state = State::Ready(Box::new(Fake(Err(()), Duration::ZERO)));
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
                    (Status::Ready, vec!["late".to_owned()])
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
