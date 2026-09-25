//! On-device language model for word prediction.
//!
//! The model spells candidate words from subword pieces, constrained to the
//! typed prefix.
//! A word scores log P(pieces | text) + log P(a word boundary follows), with
//! case variants merged. Low probability and blocked words never
//! appear. Runs only inside the prediction worker; text is never logged.
use ort::{
    session::{builder::GraphOptimizationLevel, Session, SessionInputValue},
    value::Tensor,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};
use tokenizers::Tokenizer;

const BEAM: usize = 16;
const MAX_PIECES: usize = 5;
/// Pieces below this log-probability are never explored.
const PRUNE: f32 = -18.0;
/// Minimum log-probability for a candidate word.
const UNKNOWN_MIN: f32 = -16.0;
/// Most recent buffered characters the model reads.
const CONTEXT_CHARS: usize = 256;

pub trait Scorer {
    /// Next-piece log-probabilities after `text`, which becomes the base for `extend`.
    fn context(&mut self, text: &str) -> Result<Vec<f32>, ()>;
    /// Next-piece log-probabilities after each continuation of the last context.
    fn extend(&mut self, continuations: &[Vec<i64>]) -> Result<Vec<Vec<f32>>, ()>;
}

pub struct Vocabulary {
    pieces: Vec<String>,
    /// Pieces that can end a word: space-led, punctuation or special tokens.
    boundary: Vec<bool>,
    /// Pieces made only of ASCII letters and apostrophes.
    continues: Vec<bool>,
    blocked: HashSet<String>,
}

impl Vocabulary {
    pub fn new(pieces: Vec<String>, blocked: HashSet<String>) -> Self {
        let wordy = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '’')
        };
        Self {
            boundary: pieces
                .iter()
                .map(|p| {
                    !p.chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '\'' || c == '’')
                })
                .collect(),
            continues: pieces.iter().map(|p| wordy(p)).collect(),
            pieces,
            blocked,
        }
    }
}

fn normalize(word: &str) -> String {
    word.to_lowercase().replace('’', "'")
}

/// Up to five words for `prefix` after `before`, most probable first.
/// Expansion stops at `deadline`, returning the words finished so far.
pub fn spell(
    scorer: &mut impl Scorer,
    vocabulary: &Vocabulary,
    before: &str,
    prefix: &str,
    deadline: Instant,
) -> Result<Vec<String>, ()> {
    let typed = prefix;
    let prefix = normalize(prefix);
    let text = before.trim_end();
    // A word after the last one needs a leading space; at the very start of
    // text a capital marks a word start, since fragments are never capitalised.
    let start = text.is_empty();
    if !start && text.len() == before.len() {
        return Ok(Vec::new());
    }
    let compatible = |word: &str| {
        let w = normalize(word);
        w.starts_with(&prefix) || prefix.starts_with(&w)
    };
    let next = scorer.context(text)?;
    if next.len() != vocabulary.pieces.len() {
        return Err(());
    }
    let mut beam: Vec<(f32, Vec<i64>, String)> = Vec::new();
    for (id, piece) in vocabulary.pieces.iter().enumerate() {
        let word = if start {
            piece.as_str()
        } else {
            match piece.strip_prefix(' ') {
                Some(word) => word,
                None => continue,
            }
        };
        let first = word.chars().next();
        let begins = if start {
            first.is_some_and(char::is_uppercase)
        } else {
            first.is_some_and(|c| c.is_ascii_alphabetic())
        };
        if begins
            && next[id] > PRUNE
            && word
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '’')
            && compatible(word)
        {
            beam.push((next[id], vec![id as i64], word.to_owned()));
        }
    }
    // Probability mass per normalized word, plus its most probable spelling.
    let mut finished: HashMap<String, (f32, f32, String)> = HashMap::new();
    let fifth = |finished: &HashMap<String, (f32, f32, String)>| {
        // Only words that can survive the final filter may prune the beam.
        let mut p: Vec<f32> = finished
            .iter()
            .filter(|(_, f)| f.0.ln() >= UNKNOWN_MIN)
            .map(|(_, f)| f.0)
            .collect();
        p.sort_by(|a, b| b.total_cmp(a));
        p.get(4).map_or(f32::NEG_INFINITY, |p| p.ln())
    };
    for _ in 0..MAX_PIECES {
        if Instant::now() >= deadline {
            break;
        }
        beam.sort_by(|a, b| b.0.total_cmp(&a.0));
        let bound = fifth(&finished);
        beam.retain(|b| b.0 > bound);
        beam.truncate(BEAM);
        if beam.is_empty() {
            break;
        }
        let rows = scorer.extend(&beam.iter().map(|b| b.1.clone()).collect::<Vec<_>>())?;
        if rows.len() != beam.len() {
            return Err(());
        }
        let mut grown = Vec::new();
        for ((logp, pieces, word), row) in beam.iter().zip(&rows) {
            if row.len() != vocabulary.pieces.len() {
                return Err(());
            }
            let key = normalize(word);
            let plausible = key.chars().count() > 1 || key == "a" || key == "i";
            if plausible && key.starts_with(&prefix) && !vocabulary.blocked.contains(&key) {
                let boundary: f32 = row
                    .iter()
                    .zip(&vocabulary.boundary)
                    .filter(|(_, b)| **b)
                    .map(|(l, _)| l.exp())
                    .sum();
                let score = logp + boundary.max(1e-12).ln();
                let entry = finished
                    .entry(key)
                    .or_insert((0.0, f32::NEG_INFINITY, String::new()));
                entry.0 += score.exp();
                if score > entry.1 {
                    entry.1 = score;
                    entry.2 = word.clone();
                }
            }
            for (id, piece) in vocabulary.pieces.iter().enumerate() {
                if vocabulary.continues[id] && row[id] > PRUNE {
                    let longer = format!("{word}{piece}");
                    if compatible(&longer) {
                        let mut pieces = pieces.clone();
                        pieces.push(id as i64);
                        grown.push((logp + row[id], pieces, longer));
                    }
                }
            }
        }
        beam = grown;
    }
    let mut words: Vec<(f32, String, String)> = finished
        .into_iter()
        .filter_map(|(key, (p, _, spelling))| (p.ln() >= UNKNOWN_MIN).then_some((p, key, spelling)))
        .collect();
    words.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(words
        .into_iter()
        .take(5)
        .map(|(_, key, spelling)| {
            // Sentence position and all-caps tokens should not force uppercase
            // onto ordinary completions. Keep mixed case names from the model.
            let word = if spelling.chars().skip(1).any(|c| c.is_ascii_uppercase())
                && !spelling.chars().all(|c| c.is_ascii_uppercase())
            {
                spelling.replace('’', "'")
            } else {
                key
            };
            if typed.contains('’') {
                word.replace('\'', "’")
            } else {
                word
            }
        })
        .collect())
}

struct Past {
    keys: Vec<Vec<f32>>,
    values: Vec<Vec<f32>>,
    len: usize,
}

pub struct Onnx {
    session: Session,
    tokenizer: Tokenizer,
    layers: usize,
    heads: usize,
    dim: usize,
    bos: i64,
    context: Option<(String, Past, Vec<f32>)>,
}

fn log_softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let norm = max + logits.iter().map(|l| (l - max).exp()).sum::<f32>().ln();
    logits.iter().map(|l| l - norm).collect()
}

impl Onnx {
    fn open(dir: &Path) -> Result<Self, ()> {
        // Typed text must never leave the worker, so ONNX Runtime telemetry stays off.
        ort::init().with_telemetry(false).commit();
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
        let builder = Session::builder().map_err(|_| ())?;
        let builder = builder
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| ())?;
        let mut builder = builder.with_intra_threads(threads).map_err(|_| ())?;
        let session = builder
            .commit_from_file(dir.join("model.onnx"))
            .map_err(|_| ())?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|_| ())?;
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("config.json")).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        let number = |key: &str| config[key].as_u64().map(|n| n as usize).ok_or(());
        let layers = number("num_hidden_layers")?;
        let names: HashSet<&str> = session.inputs().iter().map(|i| i.name()).collect();
        let required = ["input_ids", "attention_mask", "position_ids"];
        if !required.iter().all(|n| names.contains(n))
            || !(0..layers).all(|l| names.contains(format!("past_key_values.{l}.key").as_str()))
        {
            return Err(());
        }
        Ok(Self {
            layers,
            heads: number("num_key_value_heads")?,
            dim: number("head_dim")?,
            bos: number("bos_token_id")? as i64,
            session,
            tokenizer,
            context: None,
        })
    }

    /// Runs `batch` rows of `width` pieces after `past`; returns the last row's
    /// log-probabilities for each batch item (at `ends`) and the new past.
    fn run(
        &mut self,
        past: Option<&Past>,
        ids: Vec<i64>,
        batch: usize,
        width: usize,
        ends: &[usize],
    ) -> Result<(Vec<Vec<f32>>, Option<Past>), ()> {
        let before = past.map_or(0, |p| p.len);
        let tensor =
            |shape: [usize; 2], data: Vec<i64>| -> Result<SessionInputValue<'static>, ()> {
                Ok(Tensor::from_array((shape, data)).map_err(|_| ())?.into())
            };
        let mut inputs: Vec<(Cow<str>, SessionInputValue)> = vec![
            ("input_ids".into(), tensor([batch, width], ids)?),
            (
                "attention_mask".into(),
                tensor([batch, before + width], vec![1; batch * (before + width)])?,
            ),
            (
                "position_ids".into(),
                tensor(
                    [batch, width],
                    (0..batch)
                        .flat_map(|_| (before..before + width).map(|p| p as i64))
                        .collect(),
                )?,
            ),
        ];
        for layer in 0..self.layers {
            for (kind, cache) in [
                ("key", past.map(|p| &p.keys)),
                ("value", past.map(|p| &p.values)),
            ] {
                let data: Vec<f32> = cache.map_or(Vec::new(), |c| {
                    c[layer]
                        .iter()
                        .copied()
                        .cycle()
                        .take(c[layer].len() * batch)
                        .collect()
                });
                let value = Tensor::from_array(([batch, self.heads, before, self.dim], data))
                    .map_err(|_| ())?;
                inputs.push((
                    format!("past_key_values.{layer}.{kind}").into(),
                    value.into(),
                ));
            }
        }
        let outputs = self.session.run(inputs).map_err(|_| ())?;
        let (_, logits) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|_| ())?;
        let size = logits.len() / (batch * width);
        let rows = ends
            .iter()
            .enumerate()
            .map(|(b, end)| log_softmax(&logits[(b * width + end) * size..][..size]))
            .collect();
        let present = if batch == 1 {
            let mut keys = Vec::new();
            let mut values = Vec::new();
            for layer in 0..self.layers {
                for (kind, out) in [("key", &mut keys), ("value", &mut values)] {
                    let (_, data) = outputs[format!("present.{layer}.{kind}").as_str()]
                        .try_extract_tensor::<f32>()
                        .map_err(|_| ())?;
                    out.push(data.to_vec());
                }
            }
            Some(Past {
                keys,
                values,
                len: before + width,
            })
        } else {
            None
        };
        Ok((rows, present))
    }
}

impl Scorer for Onnx {
    fn context(&mut self, text: &str) -> Result<Vec<f32>, ()> {
        if let Some((cached, _, next)) = &self.context {
            if cached == text {
                return Ok(next.clone());
            }
        }
        self.context = None;
        let mut ids = vec![self.bos];
        let encoding = self.tokenizer.encode(text, false).map_err(|_| ())?;
        ids.extend(encoding.get_ids().iter().map(|&i| i as i64));
        let width = ids.len();
        let (mut rows, past) = self.run(None, ids, 1, width, &[width - 1])?;
        let next = rows.pop().ok_or(())?;
        self.context = Some((text.to_owned(), past.ok_or(())?, next.clone()));
        Ok(next)
    }
    fn extend(&mut self, continuations: &[Vec<i64>]) -> Result<Vec<Vec<f32>>, ()> {
        let (text, past, next) = self.context.take().ok_or(())?;
        let width = continuations.iter().map(Vec::len).max().ok_or(())?;
        let ids = continuations
            .iter()
            .flat_map(|c| c.iter().copied().chain(std::iter::repeat(0)).take(width))
            .collect();
        let ends: Vec<usize> = continuations.iter().map(|c| c.len() - 1).collect();
        let result = self.run(Some(&past), ids, continuations.len(), width, &ends);
        self.context = Some((text, past, next));
        result.map(|(rows, _)| rows)
    }
}

pub struct Model {
    onnx: Onnx,
    vocabulary: Vocabulary,
}

impl Model {
    pub fn open(dir: &Path, blocklist: &Path) -> Result<Self, ()> {
        let onnx = Onnx::open(dir)?;
        let size = onnx.tokenizer.get_vocab_size(true) as u32;
        let pieces = (0..size)
            .map(|id| onnx.tokenizer.decode(&[id], false).unwrap_or_default())
            .collect();
        let blocked = std::fs::read_to_string(blocklist)
            .map_err(|_| ())?
            .lines()
            .map(|l| normalize(l.trim()))
            .filter(|l| !l.is_empty())
            .collect();
        Ok(Self {
            onnx,
            vocabulary: Vocabulary::new(pieces, blocked),
        })
    }
}

/// Predictor behind `Database`, so tests can substitute a fake.
pub trait Predict: Send {
    fn predict(&mut self, before: &str, prefix: &str, deadline: Instant)
        -> Result<Vec<String>, ()>;
}

impl Predict for Model {
    fn predict(
        &mut self,
        before: &str,
        prefix: &str,
        deadline: Instant,
    ) -> Result<Vec<String>, ()> {
        spell(
            &mut self.onnx,
            &self.vocabulary,
            match recent(before) {
                Some(text) => text,
                None => return Ok(Vec::new()),
            },
            prefix,
            deadline,
        )
    }
}

/// At most the last `CONTEXT_CHARS` characters of `before`, starting at a
/// word boundary. This bounds the one model pass that has no deadline.
/// `None` when the kept text holds no complete word.
fn recent(before: &str) -> Option<&str> {
    let Some((cut, _)) = before.char_indices().rev().nth(CONTEXT_CHARS - 1) else {
        return Some(before);
    };
    let tail = &before[cut..];
    let start = tail.find(char::is_whitespace)?;
    let kept = tail[start..].trim_start();
    (!kept.is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pieces: 0 "<s>", 1 " Wa", 2 " wa", 3 "ter", 4 "ffle", 5 " .", 6 " water",
    /// 7 "Wa", 8 "zz", 9 " darn", 10 " x". Everything after a word is a boundary.
    struct Fake {
        next: Vec<f32>,
        contexts: usize,
    }
    fn pieces() -> Vec<String> {
        [
            "<s>", " Wa", " wa", "ter", "ffle", " .", " water", "Wa", "zz", " darn", " x",
        ]
        .map(String::from)
        .to_vec()
    }
    fn dist(pairs: &[(usize, f32)]) -> Vec<f32> {
        let mut d = vec![-30.0; 11];
        for &(i, p) in pairs {
            d[i] = p.ln();
        }
        d
    }
    impl Scorer for Fake {
        fn context(&mut self, _text: &str) -> Result<Vec<f32>, ()> {
            self.contexts += 1;
            Ok(self.next.clone())
        }
        fn extend(&mut self, continuations: &[Vec<i64>]) -> Result<Vec<Vec<f32>>, ()> {
            Ok(continuations
                .iter()
                .map(|c| match c.last() {
                    Some(1) | Some(2) | Some(7) => dist(&[(3, 0.6), (4, 0.3), (5, 0.1)]),
                    _ => dist(&[(5, 1.0)]),
                })
                .collect())
        }
    }
    fn vocabulary() -> Vocabulary {
        Vocabulary::new(pieces(), ["darn".to_owned()].into())
    }
    fn fake(pairs: &[(usize, f32)]) -> Fake {
        Fake {
            next: dist(pairs),
            contexts: 0,
        }
    }

    fn run(s: &mut Fake, before: &str, prefix: &str) -> Result<Vec<String>, ()> {
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        spell(s, &vocabulary(), before, prefix, deadline)
    }

    #[test]
    fn spells_multi_piece_words_merges_case_and_applies_prefix() {
        let mut s = fake(&[(1, 0.2), (2, 0.3), (6, 0.4), (9, 0.1)]);
        // water = 0.4 + 0.5 * 0.6; waffle = 0.5 * 0.3; bare "wa" = 0.5 * 0.1.
        let words = run(&mut s, "I would like ", "wa").unwrap();
        assert_eq!(words, ["water", "waffle", "wa"]);
        assert_eq!(run(&mut s, "I would like ", "waf").unwrap(), ["waffle"]);
    }

    #[test]
    fn blocked_single_letter_and_improbable_unknown_words_are_removed() {
        let mut s = fake(&[(9, 0.6), (10, 0.3), (6, 0.1)]);
        assert_eq!(run(&mut s, "Say ", "").unwrap(), ["water"]);
        let mut s = fake(&[(6, 1e-8)]);
        assert!(run(&mut s, "Say ", "").unwrap().is_empty());
    }

    #[test]
    fn casing_comes_from_model_except_sentence_position() {
        let mut s = fake(&[(1, 0.9)]);
        let lowered = ["water", "waffle", "wa"];
        assert_eq!(run(&mut s, "Send ", "").unwrap(), lowered);
        assert_eq!(run(&mut s, "Done. ", "").unwrap(), lowered);
    }

    #[test]
    fn text_start_needs_capital_and_attached_prefixes_fall_back() {
        let mut s = fake(&[(7, 0.5), (8, 0.5)]);
        assert_eq!(run(&mut s, "", "").unwrap(), ["water", "waffle", "wa"]);
        assert!(run(&mut s, "hello-", "wa").unwrap().is_empty());
        assert_eq!(s.contexts, 1);
    }

    #[test]
    fn an_expired_deadline_stops_expansion_and_bad_vocabularies_fail() {
        let mut s = fake(&[(6, 0.9)]);
        let expired = spell(&mut s, &vocabulary(), "Say ", "", Instant::now());
        assert!(expired.unwrap().is_empty());
        s.next.pop();
        assert!(run(&mut s, "Say ", "").is_err());
    }

    #[test]
    fn model_context_keeps_recent_whole_words_only() {
        assert_eq!(recent("I would like "), Some("I would like "));
        let long = format!("{} tea please ", "x".repeat(300));
        assert_eq!(recent(&long), Some("tea please "));
        let words = "word ".repeat(100);
        let kept = recent(&words).unwrap();
        assert!(kept.chars().count() < CONTEXT_CHARS && kept.starts_with("word"));
        assert_eq!(recent(&"é".repeat(300)), None);
        assert_eq!(recent(&format!("{} ", "x".repeat(300))), None);
    }

    #[test]
    fn apostrophes_continue_words_rather_than_ending_them() {
        let pieces = [" don", "’t", "'s", "’", " .", ","]
            .map(String::from)
            .to_vec();
        let v = Vocabulary::new(pieces, HashSet::new());
        assert_eq!(v.boundary, [true, false, false, false, true, true]);
        assert_eq!(v.continues, [false, true, true, true, false, false]);
    }

    #[test]
    fn bundled_model_suggests_current_words() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let start = Instant::now();
        let mut model = Model::open(
            &root.join("prediction-model"),
            &root.join("prediction-blocklist.txt"),
        )
        .expect("run `npm run prediction-model` to fetch the model");
        println!("model startup_ms={}", start.elapsed().as_millis());
        let mut times = Vec::new();
        for (before, prefix, expected) in [
            ("", "w", None),
            ("I would like ", "wa", None),
            ("I would like a cup of ", "", Some("tea")),
            ("Please put the ", "ket", Some("kettle")),
            ("Can you send a ", "wh", None),
            ("I want to ", "fa", None),
        ] {
            let t = Instant::now();
            let deadline = t + std::time::Duration::from_secs(60);
            let words = model.predict(before, prefix, deadline).unwrap();
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            println!("{before:?}+{prefix:?} -> {words:?}");
            assert!(words.iter().all(|w| normalize(w).starts_with(prefix)));
            if let Some(expected) = expected {
                assert!(words.iter().any(|w| w == expected), "{words:?}");
            }
        }
        println!("model predict_ms={times:.1?}");
    }
}
