//! Shared bitmaps for the native panel backgrounds and keys. Labels use platform fonts.
use crate::scanning::FrameTile;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// The colour of the menu bar band along the top of every panel.
const BAND: [u8; 3] = [30, 36, 48];

pub fn bitmap(tile: &FrameTile) -> Result<Pixmap, String> {
    use crate::scanning::TileRole;
    let style = tile.style;
    if tile.is_panel_background() {
        let mut bitmap = Pixmap::new(
            tile.rect.width.round().max(1.0) as u32,
            tile.rect.height.round().max(1.0) as u32,
        )
        .ok_or("Cannot allocate panel background")?;
        let mut paint = Paint::default();
        paint.set_color_rgba8(20, 24, 32, 255);
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
            paint.set_color_rgba8(BAND[0], BAND[1], BAND[2], 255);
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
        bitmap.fill(Color::from_rgba8(BAND[0], BAND[1], BAND[2], 255));
        return Ok(bitmap);
    }
    let on_band = style.is_some_and(|s| s.role == TileRole::MenuBar);
    let base = match style.map(|s| s.role) {
        Some(TileRole::Character) => [43, 51, 66],
        Some(TileRole::Utility) => [34, 41, 54],
        Some(TileRole::MenuBar) => [52, 61, 79],
        _ => [25, 30, 40],
    };
    bitmap.fill(if on_band {
        Color::from_rgba8(BAND[0], BAND[1], BAND[2], 255)
    } else {
        Color::from_rgba8(20, 24, 32, 255)
    });
    let rgb = if tile.selected {
        tile.color.menu_fill()
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
    paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
    bitmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    let [r, g, b] = if tile.selected {
        tile.color.rgb()
    } else if on_band {
        [74, 86, 108]
    } else {
        [62, 72, 89]
    };
    paint.set_color_rgba8(r, g, b, 255);
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
        let [r, g, b] = tile.color.rgb();
        paint.set_color_rgba8(r, g, b, 255);
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
        Some(crate::scanning::TileRole::Status) => 17.0,
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
            };
            let image = bitmap(&tile).unwrap();
            let pixel = ((28 * 176 + 88) * 4) as usize;
            assert_eq!(&image.data()[pixel..pixel + 3], &color.menu_fill());
            let previous = tile.clone();
            tile.color = if color == Blue { Red } else { Blue };
            assert_ne!(tile, previous);
            assert_ne!(bitmap(&tile).unwrap().data(), image.data());
            tile.selected = false;
            assert_ne!(
                &bitmap(&tile).unwrap().data()[pixel..pixel + 3],
                &color.menu_fill()
            );
        }
    }
}
