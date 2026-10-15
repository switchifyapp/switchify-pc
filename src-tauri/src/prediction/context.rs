use unicode_segmentation::UnicodeSegmentation;

const SENTENCE_ENDS: [char; 8] = ['.', '!', '?', '\n', '\r', '。', '！', '？'];

pub struct Context {
    pub prefix: String,
    /// The current sentence before `prefix`, without a clipped leading
    /// fragment. Statistical prediction sees only this.
    pub before: String,
    /// All buffered text before `prefix`, earlier sentences included, for
    /// generation. When the buffer was clipped it starts at its first whole
    /// sentence, or after the clipped word if no sentence ends in it, so its
    /// start moves only when a whole sentence is clipped.
    pub text: String,
    /// Clipping left no complete earlier word, so an empty `before` is not
    /// the true start of the text.
    pub partial: bool,
}
pub fn extract(text: &str, clipped: bool) -> Context {
    let context = extract_words(text, clipped);
    let boundary = text.rfind(SENTENCE_ENDS);
    let start = boundary.map_or(0, |i| i + text[i..].chars().next().unwrap().len_utf8());
    let typed = &text[..text.len() - context.prefix.len()];
    let mut before = &typed[start..];
    if clipped && start == 0 {
        before = before
            .find(char::is_whitespace)
            .map_or("", |i| &before[i..]);
    }
    let earlier = if clipped {
        match typed.find(SENTENCE_ENDS) {
            Some(i) => &typed[i + typed[i..].chars().next().unwrap().len_utf8()..],
            None => before,
        }
    } else {
        typed
    };
    Context {
        partial: clipped && start == 0 && before.trim().is_empty(),
        before: before.to_owned(),
        text: earlier.to_owned(),
        ..context
    }
}
fn extract_words(text: &str, clipped: bool) -> Context {
    let boundary = text.rfind(SENTENCE_ENDS);
    let start = boundary.map_or(0, |i| i + text[i..].chars().next().unwrap().len_utf8());
    let tail = &text[start..];
    let mut tokens: Vec<_> = tail.unicode_word_indices().collect();
    if clipped && start == 0 && tokens.first().is_some_and(|(i, _)| *i == 0) {
        tokens.remove(0);
    }
    let prefix = if tokens.last().is_some_and(|(i, w)| {
        i + w.len() == tail.len()
            || (matches!(&tail[i + w.len()..], "'" | "’")
                && w.chars().last().is_some_and(char::is_alphabetic))
    }) {
        let (i, _) = tokens.pop().unwrap();
        tail[i..].to_owned()
    } else {
        String::new()
    };
    Context {
        prefix,
        before: String::new(),
        text: String::new(),
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
        assert_eq!(
            extract("Hi. I would like wa", false).before,
            " I would like "
        );
        assert_eq!(extract("agment whole pa", true).before, " whole ");
        assert_eq!(extract("agment", true).before, "");
        assert!(extract("agment", true).partial);
        assert!(!extract("agment whole pa", true).partial);
        assert!(!extract("wa", false).partial);
        assert_eq!(extract("I think i’", false).prefix, "i’");
        assert_eq!(extract("I can'", false).prefix, "can'");
        assert_eq!(extract("Old context。I need h", false).before, "I need ");
        assert!(!extract("agment. h", true).partial);
        assert_eq!(extract("I can't", false).prefix, "can't");
    }

    #[test]
    fn generation_sees_earlier_sentences_and_statistics_do_not() {
        let c = extract("We went to the beach. It was warm! The wa", false);
        assert_eq!(c.prefix, "wa");
        assert_eq!(c.before, " The ");
        assert_eq!(c.text, "We went to the beach. It was warm! The ");
        let c = extract("Hi.\nOld context。I need h", false);
        assert_eq!(c.before, "I need ");
        assert_eq!(c.text, "Hi.\nOld context。I need ");
        // Just after a sentence end there is no current sentence yet.
        let c = extract("It was warm. ", false);
        assert_eq!((c.prefix.as_str(), c.before.as_str()), ("", " "));
        assert_eq!(c.text, "It was warm. ");
        // A clipped buffer starts at its first whole sentence.
        let c = extract("ach. It was warm. The wa", true);
        assert_eq!(c.before, " The ");
        assert_eq!(c.text, " It was warm. The ");
        assert!(!c.partial);
        let c = extract("ach. The wa", true);
        assert_eq!((c.before.as_str(), c.text.as_str()), (" The ", " The "));
        // Without a sentence end, only the clipped word is dropped.
        let c = extract("agment whole pa", true);
        assert_eq!((c.before.as_str(), c.text.as_str()), (" whole ", " whole "));
        let c = extract("agment", true);
        assert!(c.partial && c.text.is_empty());
    }
}
