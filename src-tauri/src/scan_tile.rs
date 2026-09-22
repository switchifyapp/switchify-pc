//! Shared vector artwork for the native action tiles. Labels use platform fonts.
use crate::{scan_menu::Item, scanning::FrameTile};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

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
    if tile.icon == Item::KeyboardKey {
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
        return Ok(bitmap);
    }
    let size = tile.rect.width.round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(size, size).ok_or("Cannot allocate action tile")?;
    pixmap.fill(if tile.selected {
        let [r, g, b] = tile.color.menu_fill();
        Color::from_rgba8(r, g, b, 255)
    } else {
        Color::from_rgba8(30, 35, 46, 255)
    });
    let transform = Transform::from_scale(size as f32 / 168.0, size as f32 / 168.0);
    let mut paint = Paint::default();
    let [r, g, b] = tile.color.rgb();
    paint.set_color_rgba8(r, g, b, 255);
    if tile.selected {
        let mut border = PathBuilder::new();
        let thickness = 4.0 * tile.thickness.scale() as f32;
        let inset = thickness / 2.0;
        border.push_rect(
            tiny_skia::Rect::from_xywh(inset, inset, 168.0 - thickness, 168.0 - thickness).unwrap(),
        );
        pixmap.stroke_path(
            &border.finish().unwrap(),
            &paint,
            &Stroke {
                width: thickness,
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
        TypeHere | Keyboard | More | Group(_) | Command(_) | Setting(_) | Display(_) | Pause
        | Reverse => {
            artwork(&mut path, tile.icon);
        }
        DragHere => {
            lines(&[(55., 69.), (75., 89.), (113., 47.)]);
        }
        KeyboardKey => unreachable!("Keyboard keys are rendered without artwork"),
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

fn line(path: &mut PathBuilder, points: &[(f32, f32)]) {
    path.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        path.line_to(x, y);
    }
}
fn rect(path: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32) {
    path.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h).unwrap());
}
fn arrow(path: &mut PathBuilder, x: f32, y: f32, direction: f32) {
    line(path, &[(x - 16. * direction, y), (x + 16. * direction, y)]);
    line(
        path,
        &[
            (x + 5. * direction, y - 11.),
            (x + 16. * direction, y),
            (x + 5. * direction, y + 11.),
        ],
    );
}
fn plus(path: &mut PathBuilder, x: f32, y: f32, positive: bool) {
    line(path, &[(x - 9., y), (x + 9., y)]);
    if positive {
        line(path, &[(x, y - 9.), (x, y + 9.)]);
    }
}
fn cross(path: &mut PathBuilder, x: f32, y: f32) {
    line(path, &[(x - 8., y - 8.), (x + 8., y + 8.)]);
    line(path, &[(x - 8., y + 8.), (x + 8., y - 8.)]);
}
fn monitor(path: &mut PathBuilder) {
    rect(path, 49., 39., 70., 47.);
    line(path, &[(84., 86.), (84., 98.)]);
    line(path, &[(67., 99.), (101., 99.)]);
}
fn window(path: &mut PathBuilder) {
    rect(path, 49., 40., 70., 59.);
    line(path, &[(49., 54.), (119., 54.)]);
    path.push_circle(57., 47., 1.);
}
fn tab(path: &mut PathBuilder) {
    line(
        path,
        &[
            (48., 98.),
            (48., 49.),
            (55., 49.),
            (61., 39.),
            (83., 39.),
            (89., 49.),
            (120., 49.),
            (120., 98.),
            (48., 98.),
        ],
    );
    line(path, &[(49., 59.), (119., 59.)]);
}
fn mouse(path: &mut PathBuilder, x: f32) {
    path.move_to(x, 58.);
    path.cubic_to(x, 30., x + 38., 30., x + 38., 58.);
    path.line_to(x + 38., 80.);
    path.cubic_to(x + 38., 108., x, 108., x, 80.);
    path.close();
    line(path, &[(x, 61.), (x + 38., 61.)]);
    line(path, &[(x + 19., 39.), (x + 19., 59.)]);
}
fn return_arrow(path: &mut PathBuilder, forward: bool) {
    let (start, end, direction) = if forward {
        (52., 108., 1.)
    } else {
        (116., 60., -1.)
    };
    path.move_to(start, 94.);
    path.cubic_to(start, 57., end, 57., end, 63.);
    line(
        path,
        &[
            (end - 13. * direction, 48.),
            (end + 3. * direction, 63.),
            (end - 13. * direction, 77.),
        ],
    );
}
fn artwork(path: &mut PathBuilder, item: Item) {
    use crate::scan_menu::{Command as C, Kind as K, Setting as S};
    match item {
        Item::TypeHere | Item::Keyboard => {
            rect(path, 46., 44., 76., 48.);
            for y in [55., 67.] {
                for x in [58., 74., 90., 106.] {
                    rect(path, x, y, 4., 4.);
                }
            }
            line(path, &[(66., 81.), (102., 81.)]);
            if item == Item::TypeHere {
                line(path, &[(84., 100.), (84., 114.)]);
                line(path, &[(77., 107.), (91., 107.)]);
            }
        }
        Item::More => {
            for x in [60., 84., 108.] {
                path.push_circle(x, 68., 5.);
            }
        }
        Item::Group(kind) => match kind {
            K::Mouse => mouse(path, 65.),
            K::Editing => {
                line(
                    path,
                    &[
                        (54., 95.),
                        (59., 76.),
                        (102., 33.),
                        (116., 47.),
                        (73., 90.),
                        (54., 95.),
                    ],
                );
                line(path, &[(59., 76.), (73., 90.)]);
                line(path, &[(95., 40.), (109., 54.)]);
                line(path, &[(85., 99.), (118., 99.)]);
            }
            K::Windows => {
                rect(path, 46., 36., 57., 43.);
                rect(path, 65., 58., 57., 43.);
                line(path, &[(65., 70.), (122., 70.)]);
            }
            K::Browser => {
                path.push_circle(84., 68., 32.);
                path.move_to(84., 36.);
                path.cubic_to(59., 50., 59., 86., 84., 100.);
                path.move_to(84., 36.);
                path.cubic_to(109., 50., 109., 86., 84., 100.);
                line(path, &[(52., 68.), (116., 68.)]);
            }
            K::Media => {
                path.push_circle(84., 68., 33.);
                line(path, &[(76., 51.), (101., 68.), (76., 85.), (76., 51.)]);
            }
            K::Displays => {
                monitor(path);
                rect(path, 42., 32., 70., 47.);
            }
            K::Scanning => {
                rect(path, 51., 35., 66., 66.);
                line(path, &[(73., 35.), (73., 101.)]);
                line(path, &[(95., 35.), (95., 101.)]);
                line(path, &[(51., 57.), (117., 57.)]);
                line(path, &[(51., 79.), (117., 79.)]);
                path.push_circle(84., 68., 5.);
            }
            K::Tabs => {
                tab(path);
                line(path, &[(96., 39.), (119., 39.), (126., 49.)]);
            }
            K::Zoom => {
                path.push_circle(78., 61., 24.);
                line(path, &[(95., 79.), (117., 101.)]);
            }
            K::More | K::Actions | K::Scroll | K::ConfirmDrag => {
                unreachable!("Not a grouped menu tile")
            }
        },
        Item::Command(command) => match command {
            C::MiddleClick => {
                mouse(path, 65.);
                rect(path, 80., 44., 8., 12.);
            }
            C::TripleClick => {
                mouse(path, 57.);
                for y in [43., 58., 73.] {
                    line(path, &[(108., y), (119., y)]);
                }
            }
            C::ShiftClick | C::CtrlClick | C::AltClick | C::MetaClick => {
                mouse(path, 44.);
                match command {
                    C::ShiftClick => line(
                        path,
                        &[
                            (99., 79.),
                            (99., 63.),
                            (91., 63.),
                            (109., 45.),
                            (127., 63.),
                            (119., 63.),
                            (119., 79.),
                            (99., 79.),
                        ],
                    ),
                    C::CtrlClick => line(path, &[(94., 70.), (109., 52.), (124., 70.)]),
                    C::AltClick => {
                        line(path, &[(91., 51.), (103., 51.), (116., 81.), (129., 81.)]);
                        line(path, &[(111., 51.), (129., 51.)]);
                    }
                    C::MetaClick if cfg!(target_os = "macos") => {
                        rect(path, 101., 57., 16., 16.);
                        for (x, y) in [(97., 53.), (121., 53.), (97., 77.), (121., 77.)] {
                            path.push_circle(x, y, 4.);
                        }
                    }
                    C::MetaClick => {
                        for (x, y) in [(94., 49.), (113., 49.), (94., 68.), (113., 68.)] {
                            rect(path, x, y, 14., 14.);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            C::Copy => {
                rect(path, 48., 36., 48., 52.);
                rect(path, 70., 54., 48., 52.);
            }
            C::Cut => {
                path.push_circle(60., 86., 11.);
                path.push_circle(108., 86., 11.);
                line(path, &[(68., 78.), (113., 37.)]);
                line(path, &[(100., 78.), (55., 37.)]);
            }
            C::Paste => {
                rect(path, 55., 42., 58., 60.);
                rect(path, 71., 34., 26., 16.);
                line(path, &[(70., 66.), (99., 66.)]);
                line(path, &[(70., 80.), (94., 80.)]);
            }
            C::SelectAll => {
                for (x, y, dx, dy) in [
                    (50., 37., 16., 16.),
                    (118., 37., -16., 16.),
                    (50., 99., 16., -16.),
                    (118., 99., -16., -16.),
                ] {
                    line(path, &[(x + dx, y), (x, y), (x, y + dy)]);
                }
                for y in [53., 68., 83.] {
                    line(path, &[(68., y), (101., y)]);
                }
            }
            C::Undo | C::Redo => return_arrow(path, command == C::Redo),
            C::Save => {
                line(
                    path,
                    &[
                        (54., 36.),
                        (104., 36.),
                        (116., 48.),
                        (116., 101.),
                        (54., 101.),
                        (54., 36.),
                    ],
                );
                rect(path, 68., 36., 30., 22.);
                rect(path, 66., 76., 38., 25.);
            }
            C::Find => {
                path.push_circle(76., 60., 23.);
                line(path, &[(93., 77.), (117., 101.)]);
                line(path, &[(67., 56.), (85., 56.)]);
                line(path, &[(67., 65.), (79., 65.)]);
            }
            C::SwitchNext | C::SwitchPrevious => {
                rect(path, 48., 36., 46., 38.);
                rect(path, 73., 54., 46., 38.);
                arrow(
                    path,
                    84.,
                    98.,
                    if command == C::SwitchNext { 1. } else { -1. },
                );
            }
            C::Overview => {
                rect(path, 48., 37., 30., 27.);
                rect(path, 88., 37., 32., 40.);
                rect(path, 48., 74., 30., 27.);
                rect(path, 88., 87., 32., 14.);
            }
            C::Desktop => {
                monitor(path);
                rect(path, 59., 48., 9., 9.);
                rect(path, 59., 66., 9., 9.);
                line(path, &[(77., 78.), (108., 78.)]);
            }
            C::Minimize => {
                window(path);
                line(path, &[(71., 85.), (99., 85.)]);
            }
            C::Maximize => {
                for (x, y, dx, dy) in [
                    (52., 39., 18., 18.),
                    (116., 39., -18., 18.),
                    (52., 99., 18., -18.),
                    (116., 99., -18., -18.),
                ] {
                    line(path, &[(x + dx, y), (x, y), (x, y + dy)]);
                    line(path, &[(x, y), (x + dx, y + dy)]);
                }
            }
            C::CloseWindow => {
                window(path);
                cross(path, 84., 77.);
            }
            C::BrowserBack | C::BrowserForward => {
                window(path);
                arrow(
                    path,
                    84.,
                    77.,
                    if command == C::BrowserForward {
                        1.
                    } else {
                        -1.
                    },
                );
            }
            C::Reload => {
                path.move_to(111., 52.);
                path.cubic_to(88., 24., 48., 43., 54., 76.);
                path.cubic_to(60., 105., 98., 110., 115., 85.);
                line(path, &[(112., 35.), (112., 54.), (94., 54.)]);
            }
            C::NewTab | C::CloseTab | C::ReopenTab | C::NextTab | C::PreviousTab => {
                tab(path);
                match command {
                    C::NewTab => plus(path, 84., 79., true),
                    C::CloseTab => cross(path, 84., 79.),
                    C::NextTab | C::PreviousTab => {
                        arrow(path, 84., 79., if command == C::NextTab { 1. } else { -1. })
                    }
                    C::ReopenTab => {
                        path.move_to(103., 89.);
                        path.cubic_to(103., 69., 80., 68., 72., 77.);
                        line(path, &[(72., 66.), (72., 79.), (85., 79.)]);
                    }
                    _ => unreachable!(),
                }
            }
            C::Address => {
                rect(path, 44., 48., 80., 38.);
                path.push_circle(60., 67., 5.);
                line(path, &[(79., 57.), (79., 77.)]);
                line(path, &[(74., 57.), (84., 57.)]);
                line(path, &[(74., 77.), (84., 77.)]);
                line(path, &[(96., 67.), (111., 67.)]);
            }
            C::ZoomIn | C::ZoomOut | C::ZoomReset => {
                path.push_circle(76., 60., 25.);
                line(path, &[(94., 79.), (116., 101.)]);
                if command == C::ZoomReset {
                    line(path, &[(67., 52.), (67., 68.)]);
                    line(path, &[(85., 52.), (85., 68.)]);
                    path.push_circle(76., 56., 1.);
                    path.push_circle(76., 64., 1.);
                } else {
                    plus(path, 76., 60., command == C::ZoomIn);
                }
            }
            C::PlayPause => {
                line(path, &[(52., 43.), (84., 68.), (52., 93.), (52., 43.)]);
                line(path, &[(101., 44.), (101., 92.)]);
                line(path, &[(117., 44.), (117., 92.)]);
            }
            C::NextTrack | C::PreviousTrack => {
                let d = if command == C::NextTrack { 1. } else { -1. };
                line(
                    path,
                    &[
                        (84. - 24. * d, 44.),
                        (84. + 12. * d, 68.),
                        (84. - 24. * d, 92.),
                        (84. - 24. * d, 44.),
                    ],
                );
                line(path, &[(84. + 26. * d, 44.), (84. + 26. * d, 92.)]);
            }
            C::VolumeUp | C::VolumeDown | C::Mute => {
                line(
                    path,
                    &[
                        (47., 57.),
                        (60., 57.),
                        (81., 40.),
                        (81., 96.),
                        (60., 79.),
                        (47., 79.),
                        (47., 57.),
                    ],
                );
                if command == C::Mute {
                    cross(path, 110., 68.);
                } else {
                    plus(path, 110., 68., command == C::VolumeUp);
                }
            }
        },
        Item::Display(next) => {
            monitor(path);
            arrow(path, 84., 63., if next { 1. } else { -1. });
        }
        Item::Pause => {
            rect(path, 62., 42., 13., 52.);
            rect(path, 94., 42., 13., 52.);
        }
        Item::Reverse => {
            arrow(path, 84., 51., -1.);
            arrow(path, 84., 86., 1.);
        }
        Item::Setting(setting) => match setting {
            S::FasterScan | S::SlowerScan => {
                path.push_circle(74., 68., 27.);
                line(path, &[(74., 49.), (74., 68.), (88., 76.)]);
                line(path, &[(66., 32.), (82., 32.)]);
                plus(path, 117., 43., setting == S::FasterScan);
            }
            S::FasterLine | S::SlowerLine => {
                line(path, &[(57., 38.), (57., 99.)]);
                line(path, &[(68., 88.), (97., 88.)]);
                line(path, &[(84., 77.), (97., 88.), (84., 99.)]);
                plus(path, 106., 49., setting == S::FasterLine);
            }
            S::LineMode => {
                rect(path, 50., 35., 68., 66.);
                line(path, &[(79., 36.), (79., 100.)]);
                line(path, &[(51., 74.), (117., 74.)]);
                path.push_circle(79., 74., 6.);
            }
            S::GridMode => {
                rect(path, 48., 36., 72., 64.);
                line(path, &[(72., 36.), (72., 100.)]);
                line(path, &[(96., 36.), (96., 100.)]);
                line(path, &[(48., 57.), (120., 57.)]);
                line(path, &[(48., 79.), (120., 79.)]);
                rect(path, 75., 60., 18., 16.);
            }
        },
        _ => unreachable!("Legacy tile artwork is drawn separately"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_scanner_colours_tint_selected_tiles_and_invalidate_frame_equality() {
        use crate::scanning::{Rect, ScannerColor::*};
        for color in [Red, Green, Blue, Yellow, White] {
            let mut tile = FrameTile {
                thickness: Default::default(),
                style: None,
                color,
                text: "Click".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 168.0,
                    height: 168.0,
                },
                scale: 1.0,
                icon: Item::LeftClick,
                selected: true,
            };
            let image = bitmap(&tile).unwrap();
            let pixel = (12 * 168 + 12) * 4;
            assert_eq!(&image.data()[pixel..pixel + 3], &color.menu_fill());
            let previous = tile.clone();
            tile.color = if color == Blue { Red } else { Blue };
            assert_ne!(tile, previous);
            assert_ne!(bitmap(&tile).unwrap().data(), image.data());
        }
    }
    #[test]
    fn every_action_has_distinct_artwork_and_selection_changes_the_background() {
        use crate::scan_menu::{Command as C, Kind as K, Setting as S};
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
            More,
            Group(K::Mouse),
            Group(K::Editing),
            Group(K::Windows),
            Group(K::Browser),
            Group(K::Media),
            Group(K::Displays),
            Group(K::Scanning),
            Group(K::Tabs),
            Group(K::Zoom),
            Command(C::MiddleClick),
            Command(C::TripleClick),
            Command(C::ShiftClick),
            Command(C::CtrlClick),
            Command(C::AltClick),
            Command(C::MetaClick),
            Command(C::Copy),
            Command(C::Cut),
            Command(C::Paste),
            Command(C::SelectAll),
            Command(C::Undo),
            Command(C::Redo),
            Command(C::Save),
            Command(C::Find),
            Command(C::SwitchNext),
            Command(C::SwitchPrevious),
            Command(C::Overview),
            Command(C::Desktop),
            Command(C::Minimize),
            Command(C::Maximize),
            Command(C::CloseWindow),
            Command(C::BrowserBack),
            Command(C::BrowserForward),
            Command(C::Reload),
            Command(C::NewTab),
            Command(C::CloseTab),
            Command(C::ReopenTab),
            Command(C::NextTab),
            Command(C::PreviousTab),
            Command(C::Address),
            Command(C::ZoomIn),
            Command(C::ZoomOut),
            Command(C::ZoomReset),
            Command(C::PlayPause),
            Command(C::NextTrack),
            Command(C::PreviousTrack),
            Command(C::VolumeUp),
            Command(C::VolumeDown),
            Command(C::Mute),
            Display(true),
            Display(false),
            Pause,
            Reverse,
            Setting(S::FasterScan),
            Setting(S::SlowerScan),
            Setting(S::FasterLine),
            Setting(S::SlowerLine),
            Setting(S::LineMode),
            Setting(S::GridMode),
        ] {
            let mut tile = FrameTile {
                thickness: Default::default(),
                style: None,
                color: Default::default(),
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
            assert!(
                images.iter().all(|previous| previous != normal.data()),
                "Duplicate artwork: {icon:?}"
            );
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
