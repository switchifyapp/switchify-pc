//! What a word predictor gives the worker. Only the isolated worker holds
//! typed context.

/// Prediction slots: five words.
pub const SLOTS: usize = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prediction {
    pub words: [Option<String>; SLOTS],
}

#[cfg(test)]
pub fn slots(words: impl IntoIterator<Item = impl Into<String>>) -> [Option<String>; SLOTS] {
    let mut words = words.into_iter();
    std::array::from_fn(|_| words.next().map(Into::into))
}

pub trait Predict: Send {
    fn predict(&mut self, before: &str, prefix: &str) -> Result<Prediction, ()>;
    /// Like `predict`, with where each letter sits on the keyboard, so a
    /// word may correct a press that landed on a nearby key instead of
    /// completing the prefix. A predictor that cannot only completes.
    fn predict_near(
        &mut self,
        before: &str,
        prefix: &str,
        _keys: &[super::nearby::KeyPosition],
    ) -> Result<Prediction, ()> {
        self.predict(before, prefix)
    }
    /// A refinement of the last prediction, when the predictor makes one.
    fn poll(&mut self) -> Option<Prediction> {
        None
    }
    fn pending(&self) -> bool {
        false
    }
    fn reset(&mut self) {}
}
