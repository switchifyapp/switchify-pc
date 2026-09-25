use unicode_segmentation::UnicodeSegmentation;

pub struct Context {
    pub prefix: String,
    pub sentence_start: bool,
    /// Buffered text before `prefix`, without a clipped leading fragment.
    pub before: String,
    /// Clipping left no complete earlier word, so an empty `before` is not
    /// the true start of the text.
    pub partial: bool,
}
pub fn extract(text: &str, clipped: bool) -> Context {
    let context = extract_words(text, clipped);
    let mut before = &text[..text.len() - context.prefix.len()];
    if clipped {
        before = before
            .find(char::is_whitespace)
            .map_or("", |i| &before[i..]);
    }
    Context {
        partial: clipped && before.trim().is_empty(),
        before: before.to_owned(),
        ..context
    }
}
fn extract_words(text: &str, clipped: bool) -> Context {
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
        prefix,
        before: String::new(),
        partial: false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_boundaries_and_clipped_words() {
        let c = extract("Discard this. I can’t find cafe\u{301}", false);
        assert_eq!(c.prefix, "cafe\u{301}");
        assert!(extract("Done! ", false).sentence_start);
        assert_eq!(
            extract("Hi. I would like wa", false).before,
            "Hi. I would like "
        );
        assert_eq!(extract("agment whole pa", true).before, " whole ");
        assert_eq!(extract("agment", true).before, "");
        assert!(extract("agment", true).partial);
        assert!(!extract("agment whole pa", true).partial);
        assert!(!extract("wa", false).partial);
    }
}
