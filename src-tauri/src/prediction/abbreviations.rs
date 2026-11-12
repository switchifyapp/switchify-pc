//! Opt-in words from some of their letters typed in order, such as `abt` for
//! `about`. Exact completions keep their slots; abbreviations only fill the
//! instant slots left over, ahead of nearby-key corrections.
use switchify_prediction::{KeyHitMap, KeySubstitution, Options, Predictor};

/// Instant slots, as in the model.
const INSTANT: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Candidate {
    pub word: String,
    pub substitution: Option<KeySubstitution>,
    pub abbreviated: bool,
}

/// Backspace behavior for arbitrary Unicode differs between target apps. Only
/// owned ASCII-letter prefixes of two to 32 letters are replaced whole.
pub(super) fn eligible(prefix: &str) -> bool {
    (2..=32).contains(&prefix.len()) && prefix.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Whether `word` starts with the prefix's first letter and contains the rest
/// in order, but does not simply start with the prefix.
pub(super) fn abbreviates(prefix: &str, word: &str) -> bool {
    if !eligible(prefix) {
        return false;
    }
    let typed = prefix.to_ascii_lowercase();
    let word = switchify_prediction::normalize(word);
    if word.starts_with(&typed) {
        return false;
    }
    let mut letters = word.chars();
    let mut typed = typed.chars();
    if letters.next() != typed.next() {
        return false;
    }
    typed.all(|wanted| letters.any(|c| c == wanted))
}

pub(super) fn instant(
    predictor: &Predictor,
    before: &str,
    prefix: &str,
    map: Option<&KeyHitMap>,
    abbreviations: bool,
) -> Vec<Candidate> {
    let (exact, corrected): (Vec<_>, Vec<_>) =
        super::nearby::suggestions(predictor, before, prefix, map)
            .into_iter()
            .partition(|s| s.substitution.is_none());
    let mut result: Vec<_> = exact
        .into_iter()
        .map(|s| Candidate {
            word: s.word,
            substitution: None,
            abbreviated: false,
        })
        .collect();
    if abbreviations && result.len() < INSTANT && eligible(prefix) {
        let options = Options {
            limit: 2 * INSTANT,
            min_chars: 0,
            unigram_only: false,
        };
        for s in predictor.predict_abbreviated(before, prefix, options) {
            if result.len() == INSTANT {
                break;
            }
            if s.abbreviated && !result.iter().any(|c| c.word == s.word) {
                result.push(Candidate {
                    word: s.word,
                    substitution: None,
                    abbreviated: true,
                });
            }
        }
    }
    for s in corrected {
        if result.len() == INSTANT {
            break;
        }
        if !result.iter().any(|c| c.word == s.word) {
            result.push(Candidate {
                word: s.word,
                substitution: s.substitution,
                abbreviated: false,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn letters_in_order_are_checked_against_the_owned_prefix() {
        assert!(abbreviates("abt", "about"));
        assert!(abbreviates("Abt", "About"));
        assert!(abbreviates("ABT", "about"));
        assert!(abbreviates("dont", "don't"));
        assert!(!abbreviates("abo", "about"));
        assert!(!abbreviates("bt", "about"));
        assert!(!abbreviates("atb", "about"));
        assert!(!abbreviates("a", "about"));
        assert!(!abbreviates("ab't", "about"));
        assert!(!abbreviates("cfé", "café"));
        assert!(!abbreviates(&"a".repeat(33), &"a".repeat(40)));
    }

    fn predictor(text: &str) -> (std::path::PathBuf, Predictor) {
        let root = std::env::temp_dir().join(format!("switchify-abbrev-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("test.sqlite");
        switchify_prediction::build(&path, text, "synthetic").unwrap();
        let predictor = Predictor::open(&path, None).unwrap();
        (root, predictor)
    }

    #[test]
    fn abbreviations_fill_only_unused_slots_before_nearby_corrections() {
        let (root, p) = predictor("Talk about it. About time. Abt. Absent. Help. Hello.");
        let off = instant(&p, "", "abt", None, false);
        assert_eq!(
            off.iter().map(|c| c.word.as_str()).collect::<Vec<_>>(),
            ["abt"]
        );
        let on = instant(&p, "", "abt", None, true);
        assert_eq!(
            on.iter()
                .map(|c| (c.word.as_str(), c.abbreviated))
                .collect::<Vec<_>>(),
            [("abt", false), ("about", true), ("absent", true)]
        );
        // Three exact completions leave no slot to fill.
        let exact = instant(&p, "", "a", None, true);
        assert!(exact.iter().all(|c| !c.abbreviated));
        assert!(instant(&p, "", "ab't", None, true)
            .iter()
            .all(|c| !c.abbreviated));
        let map = super::super::nearby::hit_map(crate::point_scan::KeyboardLayout::Qwerty);
        let both = instant(&p, "", "hrl", Some(&map), true);
        assert!(both
            .iter()
            .any(|c| c.word == "hello" && c.substitution.is_some() && !c.abbreviated));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_model_suggests_about_for_abt_only_when_enabled() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/prediction-model/english.ngrams");
        let p = Predictor::open(&path, None).unwrap();
        assert!(!instant(&p, "I need ", "abt", None, false)
            .iter()
            .any(|c| c.word == "about"));
        for (typed, word) in [("abt", "about"), ("ppl", "people"), ("tmrw", "tomorrow")] {
            assert!(
                instant(&p, "tell me ", typed, None, true)
                    .iter()
                    .any(|c| c.word == word && c.abbreviated),
                "{typed} -> {word}"
            );
        }
    }
}
