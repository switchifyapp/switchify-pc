use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    NoFocus,
    Protected,
    Unsupported,
    AmbiguousSelection,
    ProviderError,
}

// Text-bearing types deliberately have no Debug implementation.
#[derive(Clone, PartialEq, Eq)]
pub struct RawContext {
    pub before: String,
    pub after: String,
    pub clipped_start: bool,
    pub has_selection: bool,
    pub position: i64,
}
pub struct Context {
    pub words: Vec<String>,
    pub prefix: String,
    pub sentence_start: bool,
}
pub fn extract(text: &str, clipped: bool) -> Context {
    let boundary = text.rfind(['.', '!', '?', '\n', '\r', '。', '！', '？']);
    let start = boundary.map_or(0, |i| i + text[i..].chars().next().unwrap().len_utf8());
    let tail = &text[start..];
    let mut tokens: Vec<_> = tail.unicode_word_indices().collect();
    if clipped && start == 0 && tokens.first().is_some_and(|(i, _)| *i == 0) {
        tokens.remove(0);
    }
    let prefix = if tokens
        .last()
        .is_some_and(|(i, w)| i + w.len() == tail.len())
    {
        tokens.pop().unwrap().1.to_owned()
    } else {
        String::new()
    };
    Context {
        sentence_start: tokens.is_empty() && (!clipped || boundary.is_some()),
        words: tokens
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(|(_, w)| (*w).to_owned())
            .collect(),
        prefix,
    }
}
pub fn eligible(raw: &RawContext) -> bool {
    !raw.has_selection
        && raw.after.chars().next().is_none_or(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '.' | ','
                        | '!'
                        | '?'
                        | ':'
                        | ';'
                        | ')'
                        | ']'
                        | '}'
                        | '"'
                        | '/'
                        | '\\'
                        | '-'
                        | '…'
                )
        })
}
pub trait Adapter {
    type Target;
    fn focused(&mut self) -> Result<Self::Target, Status>;
    fn protected(&mut self, target: &Self::Target) -> Result<bool, Status>;
    fn editable(&mut self, target: &Self::Target) -> bool;
    fn same(&mut self, a: &Self::Target, b: &Self::Target) -> Result<bool, Status>;
    fn read(&mut self, target: &Self::Target) -> Result<RawContext, Status>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_boundaries_and_clipped_words() {
        let c = extract("Discard this. I can’t find cafe\u{301}", false);
        assert_eq!(c.words, ["I", "can’t", "find"]);
        assert_eq!(c.prefix, "cafe\u{301}");
        assert_eq!(extract("agment whole pa", true).words, ["whole"]);
        assert!(extract("Done! ", false).sentence_start);
    }
}
