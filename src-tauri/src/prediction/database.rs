use super::{
    context::Context,
    model::{Model, Predict},
};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

const SEARCH_BUDGET: Duration = Duration::from_millis(400);
const SLOW_CALL: Duration = Duration::from_millis(1000);

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
    pub fn predict(&mut self, context: &Context) -> (Status, Vec<String>) {
        let status = self.status();
        if status != Status::Ready || context.partial {
            return (status, Vec::new());
        }
        let State::Ready(model) = &mut self.state else {
            unreachable!()
        };
        let start = Instant::now();
        let result = model.predict(&context.before, &context.prefix, start + SEARCH_BUDGET);
        if start.elapsed() > self.slow_call || result.is_err() {
            self.state = State::Unavailable;
            return (Status::Unavailable, Vec::new());
        }
        let mut words = result.unwrap();
        words.truncate(5);
        (Status::Ready, words)
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            state: State::Ready(Box::new(FakeModel)),
            slow_call: SLOW_CALL,
        }
    }
}

#[cfg(test)]
struct FakeModel;
#[cfg(test)]
impl Predict for FakeModel {
    fn predict(
        &mut self,
        _before: &str,
        prefix: &str,
        _deadline: Instant,
    ) -> Result<Vec<String>, ()> {
        Ok(["water", "waffle", "walk"]
            .into_iter()
            .filter(|w| w.starts_with(&prefix.to_lowercase()))
            .map(str::to_owned)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Result<Vec<&'static str>, ()>, Duration);
    impl Predict for Fake {
        fn predict(
            &mut self,
            _before: &str,
            _prefix: &str,
            _deadline: Instant,
        ) -> Result<Vec<String>, ()> {
            std::thread::sleep(self.1);
            self.0
                .clone()
                .map(|w| w.into_iter().map(String::from).collect())
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
        };
        assert_eq!(db.predict(&context()), (Status::Loading, vec![]));
        tx.send(Err(())).unwrap();
        assert_eq!(db.predict(&context()), (Status::Unavailable, vec![]));
        let missing =
            std::env::temp_dir().join(format!("switchify-missing-model-{}", std::process::id()));
        let mut db = Database::open(&missing);
        let end = Instant::now() + Duration::from_secs(5);
        while db.status() == Status::Loading && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(db.predict(&context()), (Status::Unavailable, vec![]));
    }
    #[test]
    fn model_only_limit_failure_and_slow_call() {
        let mut db = Database::fixture();
        db.state = State::Ready(Box::new(Fake(
            Ok(vec!["one", "two", "three", "four", "five", "six"]),
            Duration::ZERO,
        )));
        assert_eq!(db.predict(&context()).1.len(), 5);
        db.state = State::Ready(Box::new(Fake(Err(()), Duration::ZERO)));
        assert_eq!(db.predict(&context()), (Status::Unavailable, vec![]));
        db.state = State::Ready(Box::new(Fake(Ok(vec!["late"]), Duration::from_millis(20))));
        db.slow_call = Duration::from_millis(10);
        assert_eq!(db.predict(&context()), (Status::Unavailable, vec![]));
        assert_eq!(db.predict(&context()), (Status::Unavailable, vec![]));
    }
}
