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
