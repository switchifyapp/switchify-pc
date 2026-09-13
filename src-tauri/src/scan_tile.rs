//! Shared vector artwork for the native action tiles. Labels use platform fonts.
use crate::{scan_menu::Item, scanning::FrameTile};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

pub fn bitmap(tile: &FrameTile) -> Result<Pixmap, String> {
    let size = tile.rect.width.round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(size, size).ok_or("Cannot allocate action tile")?;
    pixmap.fill(if tile.selected {
        Color::from_rgba8(112, 79, 10, 255)
    } else {
        Color::from_rgba8(30, 35, 46, 255)
    });
    let transform = Transform::from_scale(size as f32 / 168.0, size as f32 / 168.0);
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 204, 64, 255);
    if tile.selected {
        let mut border = PathBuilder::new();
        border.push_rect(tiny_skia::Rect::from_xywh(2.0, 2.0, 164.0, 164.0).unwrap());
        pixmap.stroke_path(
            &border.finish().unwrap(),
            &paint,
            &Stroke {
                width: 4.0,
                ..Default::default()
            },
            transform,
            None,
        );
    }
    let mut path = PathBuilder::new();
    let mut lines = |points: &[(f32, f32)]| {
        path.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            path.line_to(x, y);
        }
    };
    use Item::*;
    match tile.icon {
        LeftClick | RightClick | DoubleClick => {
            // Mouse body, divided buttons, and a filled button identify click type.
            lines(&[(62., 68.), (106., 68.)]);
            lines(&[(84., 42.), (84., 68.)]);
        }
        Up => {
            lines(&[(84., 96.), (84., 42.)]);
            lines(&[(62., 64.), (84., 42.), (106., 64.)]);
        }
        Down => {
            lines(&[(84., 42.), (84., 96.)]);
            lines(&[(62., 74.), (84., 96.), (106., 74.)]);
        }
        Left | Back => {
            lines(&[(112., 68.), (56., 68.)]);
            lines(&[(78., 46.), (56., 68.), (78., 90.)]);
        }
        Right => {
            lines(&[(56., 68.), (112., 68.)]);
            lines(&[(90., 46.), (112., 68.), (90., 90.)]);
        }
        Scroll => {
            lines(&[(72., 94.), (72., 42.)]);
            lines(&[(58., 56.), (72., 42.), (86., 56.)]);
            lines(&[(98., 42.), (98., 94.)]);
            lines(&[(84., 80.), (98., 94.), (112., 80.)]);
        }
        Drag => {
            lines(&[(53., 96.), (53., 80.), (98., 80.), (98., 42.)]);
            lines(&[(82., 58.), (98., 42.), (114., 58.)]);
        }
        NewPoint | DestinationAgain => {
            lines(&[(84., 34.), (84., 52.)]);
            lines(&[(84., 84.), (84., 102.)]);
            lines(&[(50., 68.), (68., 68.)]);
            lines(&[(100., 68.), (118., 68.)]);
        }
        Cancel | CancelDrag => {
            lines(&[(62., 46.), (106., 90.)]);
            lines(&[(106., 46.), (62., 90.)]);
        }
        DragHere => {
            lines(&[(55., 69.), (75., 89.), (113., 47.)]);
        }
    }
    if matches!(tile.icon, LeftClick | RightClick | DoubleClick) {
        path.move_to(62., 64.);
        path.cubic_to(62., 32., 106., 32., 106., 64.);
        path.line_to(106., 82.);
        path.cubic_to(106., 112., 62., 112., 62., 82.);
        path.close();
    }
    if matches!(tile.icon, NewPoint | DestinationAgain) {
        path.push_circle(84., 68., 24.);
    }
    if tile.icon == Drag {
        path.push_circle(53., 101., 6.);
    }
    pixmap.stroke_path(
        &path.finish().ok_or("Empty action icon")?,
        &paint,
        &Stroke {
            width: 4.0,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        },
        transform,
        None,
    );
    if matches!(tile.icon, LeftClick | RightClick | DoubleClick) {
        let x = if tile.icon == RightClick { 88.0 } else { 67.0 };
        let mut button = PathBuilder::new();
        button.push_rect(tiny_skia::Rect::from_xywh(x, 49., 13., 14.).unwrap());
        pixmap.fill_path(
            &button.finish().unwrap(),
            &paint,
            FillRule::Winding,
            transform,
            None,
        );
        if tile.icon == DoubleClick {
            let mut marks = PathBuilder::new();
            marks.move_to(115., 41.);
            marks.line_to(121., 35.);
            marks.move_to(119., 54.);
            marks.line_to(128., 52.);
            pixmap.stroke_path(
                &marks.finish().unwrap(),
                &paint,
                &Stroke {
                    width: 4.,
                    line_cap: LineCap::Round,
                    ..Default::default()
                },
                transform,
                None,
            );
        }
    }
    Ok(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_action_has_distinct_artwork_and_selection_changes_the_background() {
        use Item::*;
        let mut images = Vec::new();
        for icon in [
            LeftClick,
            RightClick,
            DoubleClick,
            Scroll,
            Drag,
            NewPoint,
            Cancel,
            Up,
            Down,
            Left,
            Right,
            DragHere,
        ] {
            let mut tile = FrameTile {
                text: icon.label().into(),
                rect: crate::scanning::Rect {
                    x: 0.,
                    y: 0.,
                    width: 168.,
                    height: 168.,
                },
                scale: 1.,
                icon,
                selected: false,
            };
            let normal = bitmap(&tile).unwrap();
            assert!(images.iter().all(|previous| previous != normal.data()));
            images.push(normal.data().to_vec());
            tile.selected = true;
            let selected = bitmap(&tile).unwrap();
            assert_ne!(normal.data(), selected.data());
            if let Ok(directory) = std::env::var("SWITCHIFY_TILE_ARTIFACT_DIR") {
                normal
                    .save_png(std::path::Path::new(&directory).join(format!("{icon:?}.png")))
                    .unwrap();
                selected
                    .save_png(
                        std::path::Path::new(&directory).join(format!("{icon:?}-selected.png")),
                    )
                    .unwrap();
            }
        }
    }
}
