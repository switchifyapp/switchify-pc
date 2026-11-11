//! Built-in letter positions, not measurements of a person's presses.
use crate::{point_scan::KeyboardLayout, scan_keyboard::Keyboard, scanning::Rect};
use switchify_prediction::{HitSuggestion, KeyHitMap, KeyPosition, Options, Predictor};

pub(super) fn positions(layout: KeyboardLayout) -> Vec<KeyPosition> {
    let keyboard = Keyboard::configured(false, Default::default()).with_layout(layout);
    let frame = keyboard.frame(
        Rect {
            x: 0.,
            y: 0.,
            width: 10_000.,
            height: 10_000.,
        },
        1.,
        Default::default(),
    );
    let letters: Vec<_> = frame
        .tiles
        .iter()
        .filter(|tile| tile.text.len() == 1 && tile.text.as_bytes()[0].is_ascii_lowercase())
        .collect();
    let left = letters
        .iter()
        .map(|t| t.rect.x)
        .fold(f64::INFINITY, f64::min);
    let top = letters
        .iter()
        .map(|t| t.rect.y)
        .fold(f64::INFINITY, f64::min);
    // The renderer scales the whole panel uniformly. One horizontal key pitch
    // is the shared unit for both axes; utility-key widths remain represented.
    let pitch = letters[0].rect.width + crate::scan_panel::KEY_GAP * letters[0].scale;
    letters
        .into_iter()
        .map(|t| KeyPosition {
            key: t.text.as_bytes()[0] as char,
            x: (t.rect.x + t.rect.width / 2. - left) / pitch,
            y: (t.rect.y + t.rect.height / 2. - top) / pitch,
        })
        .collect()
}

pub(super) fn hit_map(layout: KeyboardLayout) -> KeyHitMap {
    KeyHitMap::new(&positions(layout), 1.4, 0.45).expect("built-in letter positions are valid")
}

/// Exact completions keep their slots and order. Only unused instant slots
/// can receive a correction, and never for an empty or one-grapheme prefix.
pub(super) fn suggestions(
    predictor: &Predictor,
    before: &str,
    prefix: &str,
    map: Option<&KeyHitMap>,
) -> Vec<HitSuggestion> {
    let options = Options {
        limit: 3,
        min_chars: 0,
        unigram_only: false,
    };
    let mut result: Vec<_> = predictor
        .predict(before, prefix, options)
        .into_iter()
        .map(|s| HitSuggestion {
            word: s.word,
            score: s.score,
            language_score: s.score,
            key_weight: 1.,
            substitution: None,
        })
        .collect();
    if result.len() == 3
        || !(2..=32).contains(&prefix.len())
        || !prefix.bytes().all(|b| b.is_ascii_alphabetic())
    {
        return result;
    }
    if let Some(map) = map {
        for candidate in predictor.predict_with_hit_map(
            before,
            prefix,
            Options {
                limit: 6,
                ..options
            },
            map,
        ) {
            if candidate.substitution.is_some() && !result.iter().any(|s| s.word == candidate.word)
            {
                result.push(candidate);
                if result.len() == 3 {
                    break;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::Path,
        time::Instant,
    };

    fn sha(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn maps_use_each_layouts_rendered_letters() {
        for layout in [
            KeyboardLayout::Qwerty,
            KeyboardLayout::SimpleQwerty,
            KeyboardLayout::CommonLetters,
        ] {
            let keys = positions(layout);
            assert_eq!(keys.len(), 26);
            assert_eq!(
                keys.iter().map(|p| p.key).collect::<BTreeSet<_>>().len(),
                26
            );
            assert!(hit_map(layout)
                .neighbors("e")
                .iter()
                .any(|n| n.key == "r" || n.key == "t"));
        }
        assert_ne!(
            positions(KeyboardLayout::Qwerty)
                .iter()
                .find(|p| p.key == 'e')
                .unwrap()
                .x,
            positions(KeyboardLayout::CommonLetters)
                .iter()
                .find(|p| p.key == 'e')
                .unwrap()
                .x
        );
    }

    #[test]
    fn filling_unused_slots_preserves_exact_completions() {
        let root = std::env::temp_dir().join(format!("switchify-nearby-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("test.sqlite");
        switchify_prediction::build(
            &path,
            "I need help. I need help. Hello there. Here we go.",
            "synthetic",
        )
        .unwrap();
        let predictor = Predictor::open(&path, None).unwrap();
        let map = hit_map(KeyboardLayout::Qwerty);
        let exact = suggestions(&predictor, "I need ", "he", None);
        let nearby = suggestions(&predictor, "I need ", "he", Some(&map));
        assert_eq!(&nearby[..exact.len()], &exact);
        assert!(suggestions(&predictor, "I need ", "hr", Some(&map))
            .iter()
            .any(|s| s.word == "help" && s.substitution.is_some()));
        for prefix in ["", "h", "h?", "he\u{301}", "can?"] {
            assert_eq!(
                suggestions(&predictor, "", prefix, None),
                suggestions(&predictor, "", prefix, Some(&map))
            );
        }
        drop(predictor);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "synthetic layout measurement; use scripts/benchmark-nearby-layouts.py"]
    fn layout_benchmark() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/prediction-model/english.ngrams");
        let model_sha = sha(&fs::read(&path).unwrap());
        assert_eq!(
            model_sha,
            "c6df388c845354a059333b1e4abe68719e5a7016e8514c44581e14ed16bf964a"
        );
        let predictor = Predictor::open(&path, None).unwrap();
        let fixtures: Vec<(String, String)> =
            serde_json::from_str(include_str!("../../../scripts/nearby-layout-fixtures.json"))
                .unwrap();
        let mut rows = Vec::new();
        for layout in [
            KeyboardLayout::Qwerty,
            KeyboardLayout::SimpleQwerty,
            KeyboardLayout::CommonLetters,
        ] {
            let map = hit_map(layout);
            let picture = Keyboard::layout_picture(false, layout);
            let mut cases = BTreeSet::new();
            for (before, target) in &fixtures {
                for length in 2..=4.min(target.len()) {
                    let prefix = &target[..length];
                    cases.insert(("clean", before.clone(), prefix.to_owned(), target.clone()));
                    for (index, key) in prefix.chars().enumerate() {
                        let mut replacements: BTreeMap<&str, Vec<char>> = BTreeMap::new();
                        replacements.insert(
                            "geometry",
                            map.neighbors(&key.to_string())
                                .iter()
                                .map(|n| n.key.chars().next().unwrap())
                                .collect(),
                        );
                        let mut row_errors = Vec::new();
                        for (r, row) in picture.iter().enumerate() {
                            if let Some(c) =
                                row.iter().position(|(label, _)| label == &key.to_string())
                            {
                                for adjacent in
                                    [r.checked_sub(1), Some(r + 1)].into_iter().flatten()
                                {
                                    if let Some((label, _)) =
                                        picture.get(adjacent).and_then(|row| row.get(c))
                                    {
                                        if label.len() == 1
                                            && label.as_bytes()[0].is_ascii_lowercase()
                                        {
                                            row_errors.push(label.as_bytes()[0] as char);
                                        }
                                    }
                                }
                            }
                        }
                        replacements.insert("scan_row", row_errors);
                        for (kind, alternatives) in replacements {
                            for alternative in alternatives {
                                let mut noisy = prefix.as_bytes().to_vec();
                                noisy[index] = alternative as u8;
                                cases.insert((
                                    kind,
                                    before.clone(),
                                    String::from_utf8(noisy).unwrap(),
                                    target.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            for kind in ["clean", "geometry", "scan_row"] {
                let subset: Vec<_> = cases.iter().filter(|c| c.0 == kind).collect();
                let cases_sha = sha(&serde_json::to_vec(&subset).unwrap());
                for policy in ["exact", "weighted", "fill_missing"] {
                    let mut hits = 0;
                    let mut timings = Vec::new();
                    let predict = |before: &str, prefix: &str| match policy {
                        "weighted" => predictor.predict_with_hit_map(
                            before,
                            prefix,
                            Options {
                                limit: 3,
                                min_chars: 0,
                                unigram_only: false,
                            },
                            &map,
                        ),
                        "fill_missing" => suggestions(&predictor, before, prefix, Some(&map)),
                        _ => suggestions(&predictor, before, prefix, None),
                    };
                    for (_, before, prefix, _) in subset.iter().cycle().take(20) {
                        predict(before, prefix);
                    }
                    for (_, before, prefix, target) in &subset {
                        let started = Instant::now();
                        let results = predict(before, prefix);
                        timings.push(started.elapsed().as_secs_f64() * 1000.);
                        hits +=
                            usize::from(results.iter().any(|s| s.word.as_str() == target.as_str()));
                    }
                    timings.sort_by(f64::total_cmp);
                    rows.push(serde_json::json!({"layout":layout,"errors":kind,"policy":policy,"queries":subset.len(),"top3_hits":hits,
                        "cases_sha256":cases_sha,"p50_ms":timings[(timings.len()-1)/2],"p95_ms":timings[(timings.len()*95).div_ceil(100)-1]}));
                }
            }
        }
        let map = hit_map(KeyboardLayout::Qwerty);
        let mut probes = Vec::new();
        for prefix in ["", "s", "he", "hel", "help"] {
            let mut samples = Vec::new();
            for _ in 0..25 {
                let t = Instant::now();
                suggestions(&predictor, "I need ", prefix, Some(&map));
                samples.push(t.elapsed().as_secs_f64() * 1000.);
            }
            samples.sort_by(f64::total_cmp);
            probes.push(serde_json::json!({"prefix_graphemes":prefix.len(),"p50_ms":samples[12],"p95_ms":samples[23]}));
        }
        let report = serde_json::json!({"protocol":"pc-nearby-v1","synthetic":true,"model_sha256":model_sha,
            "source_hashes":{"nearby.rs":sha(include_bytes!("nearby.rs")),"scan_keyboard.rs":sha(include_bytes!("../scan_keyboard.rs")),
                "scan_panel.rs":sha(include_bytes!("../scan_panel.rs")),"fixtures":sha(include_bytes!("../../../scripts/nearby-layout-fixtures.json"))},
            "radius":1.4,"spread":0.45,"rows":rows,"short_prefix_probes":probes,
            "limitations":["Synthetic engineering fixtures, not held-out user data or evidence of user benefit.",
                "Built-in default geometry only; custom tiles, press timing and user-specific error rates are not modeled.",
                "Scan-row cases select an adjacent built-in row at the same scanner column; utility-key outcomes are omitted.",
                "Timings cover statistical prediction only; no neural inference, input injection, UI or process startup."]});
        fs::write(
            std::env::var_os("SWITCHIFY_NEARBY_REPORT").expect("runner must supply report path"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}
