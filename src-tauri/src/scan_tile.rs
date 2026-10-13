//! Shared bitmaps for the native panel backgrounds and keys. Labels use platform fonts.
use crate::scanning::FrameTile;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

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
        return Ok(bitmap);
    }
    let mut bitmap = Pixmap::new(
        tile.rect.width.round().max(1.0) as u32,
        tile.rect.height.round().max(1.0) as u32,
    )
    .ok_or("Cannot allocate keyboard key")?;
    let base = match style.map(|s| s.role) {
        Some(TileRole::Character) => [43, 51, 66],
        Some(TileRole::Utility) => [34, 41, 54],
        _ => [25, 30, 40],
    };
    bitmap.fill(Color::from_rgba8(20, 24, 32, 255));
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
    let radius = (8.0 * tile.scale) as f32;
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
        Some(crate::scanning::TileRole::Status) => 17.0,
        _ => 16.0,
    }
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
