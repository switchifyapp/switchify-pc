use unicode_segmentation::UnicodeSegmentation;

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
