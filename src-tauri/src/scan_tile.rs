//! Shared bitmaps for the native panel backgrounds and keys. Labels use platform fonts.
use crate::scanning::FrameTile;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// The colour of a tile's label, for the platform text painters.
pub fn text_rgb(tile: &FrameTile) -> [u8; 3] {
    tile.appearance.palette().text
}

pub fn bitmap(tile: &FrameTile) -> Result<Pixmap, String> {
    use crate::scanning::TileRole;
    let style = tile.style;
    let palette = tile.appearance.palette();
    let rgb = |[r, g, b]: [u8; 3]| Color::from_rgba8(r, g, b, 255);
    if tile.is_panel_background() {
        let mut bitmap = Pixmap::new(
            tile.rect.width.round().max(1.0) as u32,
            tile.rect.height.round().max(1.0) as u32,
        )
        .ok_or("Cannot allocate panel background")?;
        let mut paint = Paint::default();
        paint.set_color(rgb(palette.panel));
        let path = key_outline(
            bitmap.width() as f32,
            bitmap.height() as f32,
            0.0,
            (18.0 * tile.scale) as f32,
        );
        bitmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        // The menu bar band along the top, inside the rounded corners.
        let band = (crate::scan_panel::MENU_BAR_HEIGHT * tile.scale) as f32;
        if band < bitmap.height() as f32 {
            paint.set_color(rgb(palette.band));
            let band = band_outline(bitmap.width() as f32, band, (18.0 * tile.scale) as f32);
            bitmap.fill_path(
                &band,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        return Ok(bitmap);
    }
    let mut bitmap = Pixmap::new(
        tile.rect.width.round().max(1.0) as u32,
        tile.rect.height.round().max(1.0) as u32,
    )
    .ok_or("Cannot allocate keyboard key")?;
    // The title sits flat on the band.
    if style.is_some_and(|s| s.role == TileRole::Title) {
        bitmap.fill(rgb(palette.band));
        return Ok(bitmap);
    }
    // An empty slot: the panel colour with a faint dashed outline where a key
    // would be, so the row still reads as a row of slots.
    if style.is_some_and(|s| s.role == TileRole::Placeholder) {
        bitmap.fill(rgb(palette.panel));
        let width = bitmap.width() as f32;
        let height = bitmap.height() as f32;
        let stroke_width = tile.scale as f32;
        let inset = ((2.0 * tile.scale) as f32).max(stroke_width / 2.0);
        let path = key_outline(
            width,
            height,
            inset.min(width.min(height) / 4.0),
            (8.0 * tile.scale) as f32,
        );
        let mut paint = Paint::default();
        let [r, g, b] = palette.border;
        paint.set_color(Color::from_rgba8(r, g, b, PLACEHOLDER_ALPHA));
        let dash = (4.0 * tile.scale) as f32;
        bitmap.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: stroke_width,
                dash: tiny_skia::StrokeDash::new(vec![dash, dash], 0.0),
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
        return Ok(bitmap);
    }
    let on_band = style.is_some_and(|s| s.role == TileRole::MenuBar);
    let base = match style.map(|s| s.role) {
        Some(TileRole::Character) => palette.character,
        Some(TileRole::Utility) => palette.utility,
        Some(TileRole::MenuBar) => palette.menu_bar,
        _ => palette.other,
    };
    bitmap.fill(if on_band {
        rgb(palette.band)
    } else {
        rgb(palette.panel)
    });
    let fill = if tile.selected {
        tile.appearance.highlight_fill(tile.color)
    } else {
        base
    };
    let stroke_width = if tile.selected && !style.is_some_and(|s| s.row_scan) {
        3.0
    } else {
        1.0
    } * tile.scale as f32
        * if tile.selected {
            tile.thickness.scale() as f32
        } else {
            1.0
        };
    let inset = ((2.0 * tile.scale) as f32).max(stroke_width / 2.0);
    let radius = (if on_band { 6.0 } else { 8.0 } * tile.scale) as f32;
    let width = bitmap.width() as f32;
    let height = bitmap.height() as f32;
    let path = key_outline(width, height, inset.min(width.min(height) / 4.0), radius);
    let mut paint = Paint::default();
    paint.set_color(rgb(fill));
    bitmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    paint.set_color(rgb(if tile.selected {
        tile.appearance.highlight(tile.color)
    } else if on_band {
        palette.band_border
    } else {
        palette.border
    }));
    bitmap.stroke_path(
        &path,
        &paint,
        &Stroke {
            width: stroke_width,
            ..Default::default()
        },
        Transform::identity(),
        None,
    );
    if style.is_some_and(|s| s.active) {
        paint.set_color(rgb(tile.appearance.highlight(tile.color)));
        if let Some(rect) = tiny_skia::Rect::from_xywh(
            width * 0.35,
            height - 5.0 * tile.scale as f32,
            width * 0.3,
            (2.0 * tile.scale) as f32,
        ) {
            bitmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
    Ok(bitmap)
}

/// The opacity of a placeholder slot's outline over the panel.
const PLACEHOLDER_ALPHA: u8 = 150;

/// The smallest a key label shrinks to keep a long word on one line.
pub const MIN_LABEL_SHRINK: f64 = 0.75;
/// The smallest a key label shrinks when a word has to be split over two
/// lines instead.
const MIN_SPLIT_SHRINK: f64 = 0.6;
/// The mark at the end of a split word's first part.
pub const HYPHEN: &str = "-";

/// How much to scale a key's font so its widest word fits `available` pixels,
/// given that word's width and the hyphen's width at full size. 1.0 when the
/// word already fits. A word that would need less than [`MIN_LABEL_SHRINK`]
/// to stay on one line is split over two lines by [`break_long_words`]
/// instead, so the scale only drops as far as two hyphenated halves need.
pub fn label_shrink(widest_word: f64, hyphen: f64, available: f64) -> f64 {
    if widest_word <= 0.0 || available <= 0.0 || widest_word <= available {
        return 1.0;
    }
    // Platform measurers round widths up to whole pixels, so aim a little
    // inside the key; otherwise a word meant to shrink onto one line measures
    // a pixel too wide at the shrunk size and gets split instead.
    let target = (available - LABEL_TOLERANCE).max(1.0);
    let one_line = target / widest_word;
    if one_line >= MIN_LABEL_SHRINK {
        return one_line;
    }
    (2.0 * target / (widest_word + hyphen.max(0.0))).clamp(MIN_SPLIT_SHRINK, 1.0)
}

/// Pixels of slack between a shrunk label and the key padding, covering the
/// whole-pixel rounding of platform text measurement.
const LABEL_TOLERANCE: f64 = 1.0;

/// Splits any word wider than `available` into hyphenated parts on separate
/// lines, measured with the font that will draw them. Words that fit, spaces
/// and existing line breaks are kept as they are.
pub fn break_long_words(
    text: &str,
    available: f64,
    mut measure: impl FnMut(&str) -> f64,
) -> String {
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let mut words = Vec::new();
        for word in line.split(' ') {
            if word.is_empty() || measure(word) <= available {
                words.push(word.to_owned());
                continue;
            }
            let mut rest = word;
            let mut parts = Vec::new();
            while measure(rest) > available {
                // The longest head that fits with its hyphen, at least one
                // character and never the whole remainder.
                let mut cut = rest.chars().next().map_or(0, char::len_utf8);
                for (index, _) in rest.char_indices().skip(1) {
                    if measure(&format!("{}{HYPHEN}", &rest[..index])) <= available {
                        cut = index;
                    } else {
                        break;
                    }
                }
                if cut >= rest.len() {
                    break;
                }
                parts.push(format!("{}{HYPHEN}", &rest[..cut]));
                rest = &rest[cut..];
            }
            parts.push(rest.to_owned());
            words.push(parts.join("\n"));
        }
        lines.push(words.join(" "));
    }
    lines.join("\n")
}

pub fn keyboard_font_size(tile: &FrameTile) -> f64 {
    match tile.style.map(|s| s.role) {
        Some(crate::scanning::TileRole::Character) if tile.text.chars().count() == 1 => 24.0,
        // Key glyphs such as ⌫ share the letter size. Other single-character
        // utility text, such as a one-letter prediction, keeps the utility size.
        Some(crate::scanning::TileRole::Utility)
            if crate::scan_keyboard::Keyboard::LARGE_GLYPHS.contains(&tile.text.as_str()) =>
        {
            24.0
        }
        Some(crate::scanning::TileRole::MenuBar | crate::scanning::TileRole::Title) => 14.0,
        _ => 16.0,
    }
}
/// The top `height` of a rounded panel: round top corners, square bottom.
fn band_outline(width: f32, height: f32, radius: f32) -> tiny_skia::Path {
    let r = radius.min(width / 2.0).min(height);
    let mut p = PathBuilder::new();
    p.move_to(r, 0.0);
    p.line_to(width - r, 0.0);
    p.quad_to(width, 0.0, width, r);
    p.line_to(width, height);
    p.line_to(0.0, height);
    p.line_to(0.0, r);
    p.quad_to(0.0, 0.0, r, 0.0);
    p.close();
    p.finish().unwrap()
}
fn key_outline(width: f32, height: f32, inset: f32, radius: f32) -> tiny_skia::Path {
    let (left, top, right, bottom) = (inset, inset, width - inset, height - inset);
    let r = radius.min((right - left) / 2.0).min((bottom - top) / 2.0);
    let mut p = PathBuilder::new();
    p.move_to(left + r, top);
    p.line_to(right - r, top);
    p.quad_to(right, top, right, top + r);
    p.line_to(right, bottom - r);
    p.quad_to(right, bottom, right - r, bottom);
    p.line_to(left + r, bottom);
    p.quad_to(left, bottom, left, bottom - r);
    p.line_to(left, top + r);
    p.quad_to(left, top, left + r, top);
    p.close();
    p.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_glyph_utility_keys_use_the_letter_font_size() {
        use crate::scanning::{Rect, ScannerColor, TileRole, TileStyle};
        let tile = |role, text: &str| FrameTile {
            thickness: Default::default(),
            style: Some(TileStyle {
                role,
                active: false,
                row_scan: false,
            }),
            color: ScannerColor::default(),
            text: text.into(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 56.0,
            },
            scale: 1.0,
            selected: false,
            appearance: Default::default(),
        };
        assert_eq!(
            keyboard_font_size(&tile(TileRole::Utility, "\u{232B}")),
            24.0
        );
        // Arrow keys on the Navigation page are single glyphs too.
        assert_eq!(
            keyboard_font_size(&tile(TileRole::Utility, "\u{2191}")),
            24.0
        );
        assert_eq!(keyboard_font_size(&tile(TileRole::Character, "a")), 24.0);
        assert_eq!(keyboard_font_size(&tile(TileRole::Utility, "Esc")), 16.0);
        // A one-letter prediction is utility text, not a key glyph.
        assert_eq!(keyboard_font_size(&tile(TileRole::Utility, "I")), 16.0);
        // Shift keeps one size whether or not its state text is showing.
        for shift in [
            "\u{2B06}",
            "\u{21E7}",
            "\u{2B06}\nNext key",
            "\u{21E7}\nLocked",
        ] {
            assert_eq!(keyboard_font_size(&tile(TileRole::Utility, shift)), 16.0);
        }
        assert_eq!(keyboard_font_size(&tile(TileRole::Toolbar, "•")), 16.0);
    }

    #[test]
    fn long_words_shrink_to_one_line_down_to_a_floor_then_split_over_two() {
        assert_eq!(label_shrink(40.0, 4.0, 52.0), 1.0);
        assert_eq!(label_shrink(52.0, 4.0, 52.0), 1.0);
        // One line, aiming a pixel inside the key so whole-pixel measurement
        // of the shrunk font still fits.
        assert!((label_shrink(65.0, 4.0, 52.0) - 51.0 / 65.0).abs() < 1e-9);
        // Too wide for one readable line: two halves with a hyphen instead.
        let split = label_shrink(115.0, 4.0, 52.0);
        assert!(split > MIN_LABEL_SHRINK && split < 1.0, "{split}");
        assert!((split - 102.0 / 119.0).abs() < 1e-9);
        // A short overflow that would split keeps the full size for its halves.
        assert_eq!(label_shrink(72.0, 4.0, 52.0), 1.0);
        // Absurdly long words stop shrinking and take more lines.
        assert_eq!(label_shrink(400.0, 4.0, 52.0), 0.6);
        // Degenerate measurements never divide by zero or grow the text.
        assert_eq!(label_shrink(0.0, 4.0, 52.0), 1.0);
        assert_eq!(label_shrink(40.0, 4.0, 0.0), 1.0);
    }

    #[test]
    fn words_wider_than_the_key_split_with_hyphens_and_others_stay() {
        // Seven pixels a character, like a bold label.
        let measure = |text: &str| text.chars().count() as f64 * 7.0;
        assert_eq!(break_long_words("water", 52.0, measure), "water");
        assert_eq!(break_long_words("thank you", 52.0, measure), "thank you");
        assert_eq!(
            break_long_words("\u{2B06}\nNext key", 52.0, measure),
            "\u{2B06}\nNext key"
        );
        // Seven characters fit; six plus the hyphen make the first part.
        assert_eq!(
            break_long_words("congratulations", 52.0, measure),
            "congra-\ntulati-\nons"
        );
        assert_eq!(
            break_long_words("misunderstanding everything", 120.0, measure),
            "misunderstanding everything"
        );
        assert_eq!(
            break_long_words("misunderstanding everything", 52.0, measure),
            "misund-\nerstan-\nding everyt-\nhing"
        );
        // A key too narrow for even one character never loops forever.
        assert_eq!(break_long_words("ab", 5.0, measure), "a-\nb");
        assert_eq!(break_long_words("a", 5.0, measure), "a");
        // Multi-byte characters split on character boundaries.
        assert_eq!(break_long_words("ääääääää", 52.0, measure), "ääääää-\nää");
    }

    #[test]
    fn placeholder_slots_draw_a_faint_outline_on_the_panel_and_never_a_key_face() {
        use crate::{
            panel_theme::Appearance,
            scanning::{Rect, ScannerColor, TileRole, TileStyle},
        };
        let tile = |appearance, selected| FrameTile {
            thickness: Default::default(),
            style: Some(TileStyle {
                role: TileRole::Placeholder,
                active: false,
                row_scan: false,
            }),
            color: ScannerColor::Red,
            text: String::new(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 56.0,
            },
            scale: 1.0,
            selected,
            appearance,
        };
        let at = |image: &Pixmap, x: usize, y: usize| {
            let pixel = (y * 64 + x) * 4;
            [
                image.data()[pixel],
                image.data()[pixel + 1],
                image.data()[pixel + 2],
            ]
        };
        for appearance in [Appearance::Dark, Appearance::Light] {
            let palette = appearance.palette();
            let image = bitmap(&tile(appearance, false)).unwrap();
            // The middle is the bare panel, not a key face.
            assert_eq!(at(&image, 32, 28), palette.panel);
            assert_ne!(at(&image, 32, 28), palette.utility);
            // The outline is drawn, dashed, so some edge pixels differ from
            // the panel and some do not.
            let edge: Vec<_> = (6..58).map(|x| at(&image, x, 2)).collect();
            assert!(edge.iter().any(|&p| p != palette.panel), "{appearance:?}");
            assert!(edge.contains(&palette.panel), "{appearance:?}");
            // The outline is a faint border tint, never the solid border.
            assert!(edge.iter().all(|&p| p != palette.border), "{appearance:?}");
            // Every pixel stays opaque so the layered window composites cleanly.
            assert!(image.data().chunks_exact(4).all(|p| p[3] == 255));
            // A placeholder is never highlighted, whatever the frame says.
            assert_eq!(
                bitmap(&tile(appearance, true)).unwrap().data(),
                image.data()
            );
        }
    }

    #[test]
    fn all_scanner_colours_tint_selected_keys_and_invalidate_frame_equality() {
        use crate::scanning::{Rect, ScannerColor::*, TileRole, TileStyle};
        for color in [Red, Green, Blue, Yellow, White] {
            let mut tile = FrameTile {
                thickness: Default::default(),
                style: Some(TileStyle {
                    role: TileRole::Utility,
                    active: false,
                    row_scan: false,
                }),
                color,
                text: "Left click".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 176.0,
                    height: 56.0,
                },
                scale: 1.0,
                selected: true,
                appearance: Default::default(),
            };
            let image = bitmap(&tile).unwrap();
            let pixel = ((28 * 176 + 88) * 4) as usize;
            assert_eq!(
                &image.data()[pixel..pixel + 3],
                &crate::panel_theme::Appearance::Dark.highlight_fill(color)
            );
            let previous = tile.clone();
            tile.color = if color == Blue { Red } else { Blue };
            assert_ne!(tile, previous);
            assert_ne!(bitmap(&tile).unwrap().data(), image.data());
            tile.selected = false;
            assert_ne!(
                &bitmap(&tile).unwrap().data()[pixel..pixel + 3],
                &crate::panel_theme::Appearance::Dark.highlight_fill(color)
            );
        }
    }

    #[test]
    fn light_panels_invert_the_palette_and_deepen_the_highlight() {
        use crate::{
            panel_theme::Appearance,
            scanning::{Rect, ScannerColor, TileRole, TileStyle},
        };
        let tile = |appearance, role, selected| FrameTile {
            thickness: Default::default(),
            style: Some(TileStyle {
                role,
                active: false,
                row_scan: false,
            }),
            color: ScannerColor::White,
            appearance,
            text: "a".into(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 56.0,
            },
            scale: 1.0,
            selected,
        };
        let at = |image: &Pixmap, x: usize, y: usize| {
            let pixel = (y * 64 + x) * 4;
            [
                image.data()[pixel],
                image.data()[pixel + 1],
                image.data()[pixel + 2],
            ]
        };
        let luma =
            |[r, g, b]: [u8; 3]| u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
        // The panel behind the keys, and every key face, flips from dark to light.
        let dark = bitmap(&tile(Appearance::Dark, TileRole::Panel, false)).unwrap();
        let light = bitmap(&tile(Appearance::Light, TileRole::Panel, false)).unwrap();
        assert!(luma(at(&light, 32, 40)) > luma(at(&dark, 32, 40)));
        for role in [TileRole::Character, TileRole::Utility, TileRole::MenuBar] {
            let dark = bitmap(&tile(Appearance::Dark, role, false)).unwrap();
            let light = bitmap(&tile(Appearance::Light, role, false)).unwrap();
            assert!(luma(at(&light, 32, 28)) > luma([200, 200, 200]), "{role:?}");
            assert!(luma(at(&dark, 32, 28)) < luma([80, 80, 80]), "{role:?}");
            // The surround outside the rounded key follows the panel too.
            assert!(luma(at(&light, 0, 0)) > luma(at(&dark, 0, 0)), "{role:?}");
            assert_ne!(dark, light);
        }
        // A white highlight on a light key is a grey tint, never white on white.
        let selected = bitmap(&tile(Appearance::Light, TileRole::Utility, true)).unwrap();
        assert_eq!(
            at(&selected, 32, 28),
            Appearance::Light.highlight_fill(ScannerColor::White)
        );
        assert!(luma(at(&selected, 32, 28)) < luma([255, 255, 255]));
        assert_eq!(
            text_rgb(&tile(Appearance::Light, TileRole::Utility, false)),
            [20, 24, 32]
        );
        assert_eq!(
            text_rgb(&tile(Appearance::Dark, TileRole::Utility, false)),
            [255, 255, 255]
        );
    }
}
