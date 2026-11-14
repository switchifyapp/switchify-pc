//! Where each letter sits on Switchify's keyboard, so a suggestion can correct
//! a press that landed on a nearby key. Built-in positions, not measurements
//! of anyone's presses.
use crate::{point_scan::KeyboardLayout, scan_keyboard::Keyboard, scanning::Rect};
use serde::Serialize;

/// A letter key's centre, in key widths from the top left of the letters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct KeyPosition(pub char, pub f64, pub f64);

/// Typed prefixes a suggestion may replace rather than complete. One letter
/// says too little, and only plain letters are retyped, so Backspace removes
/// exactly what Switchify typed. The bridge's search slows as the prefix
/// grows, so longer prefixes are only completed (its NEARBY_MAX_PREFIX).
pub fn correctable(prefix: &str) -> bool {
    (2..=8).contains(&prefix.len()) && prefix.bytes().all(|b| b.is_ascii_alphabetic())
}

/// The letter keys of `layout` on the Letters page.
pub fn positions(layout: KeyboardLayout) -> Vec<KeyPosition> {
    let keyboard =
        Keyboard::configured(cfg!(target_os = "macos"), Default::default()).with_layout(layout);
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
    let Some(first) = letters.first() else {
        return Vec::new();
    };
    let left = letters
        .iter()
        .map(|t| t.rect.x)
        .fold(f64::INFINITY, f64::min);
    let top = letters
        .iter()
        .map(|t| t.rect.y)
        .fold(f64::INFINITY, f64::min);
    // The renderer scales the whole panel uniformly. One horizontal key pitch
    // is the unit for both axes.
    let pitch = first.rect.width + crate::scan_panel::KEY_GAP * first.scale;
    let mut seen = std::collections::HashSet::new();
    letters
        .into_iter()
        .filter(|t| seen.insert(t.text.clone()))
        .map(|t| {
            KeyPosition(
                t.text.as_bytes()[0] as char,
                (t.rect.x + t.rect.width / 2. - left) / pitch,
                (t.rect.y + t.rect.height / 2. - top) / pitch,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(keys: &[KeyPosition], letter: char) -> (f64, f64) {
        let key = keys.iter().find(|k| k.0 == letter).unwrap();
        (key.1, key.2)
    }

    #[test]
    fn every_layout_places_each_letter_once_in_key_widths() {
        for layout in [
            KeyboardLayout::Qwerty,
            KeyboardLayout::SimpleQwerty,
            KeyboardLayout::CommonLetters,
        ] {
            let keys = positions(layout);
            let mut letters: Vec<_> = keys.iter().map(|k| k.0).collect();
            letters.sort();
            assert_eq!(
                letters.iter().collect::<String>(),
                "abcdefghijklmnopqrstuvwxyz",
                "{layout:?}"
            );
        }
        let qwerty = positions(KeyboardLayout::Qwerty);
        let (qx, qy) = at(&qwerty, 'q');
        let (wx, wy) = at(&qwerty, 'w');
        assert!((wx - qx - 1.).abs() < 1e-9 && (wy - qy).abs() < 1e-9);
        // The next row down is about one key away, not across the keyboard.
        let (ax, ay) = at(&qwerty, 'a');
        assert!((0.5..1.5).contains(&(ay - qy)) && (ax - qx).abs() < 1.);
    }

    #[test]
    fn only_plain_letter_prefixes_of_two_to_eight_can_be_replaced() {
        assert!(correctable("hr"));
        assert!(correctable("Hrl"));
        assert!(correctable(&"a".repeat(8)));
        for prefix in ["", "h", "can'", "cafe\u{301}", "a1", &"a".repeat(9)] {
            assert!(!correctable(prefix), "{prefix:?}");
        }
    }
}
