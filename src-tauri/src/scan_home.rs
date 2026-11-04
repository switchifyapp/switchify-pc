//! Shared, font-independent Home icons. Only presentation; no input or state.
use crate::scanning::{FrameTile, HomeIcon, Rect, TileRole};
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub fn icon_rect(tile: &FrameTile) -> Option<Rect> {
    let s = tile.scale;
    match tile.style?.role {
        TileRole::HomeAction(_) => Some(Rect {
            x: (tile.rect.width - 24.0 * s) / 2.0,
            y: 10.0 * s,
            width: 24.0 * s,
            height: 24.0 * s,
        }),
        TileRole::HomeNavigation(_) => Some(Rect {
            x: 14.0 * s,
            y: (tile.rect.height - 18.0 * s) / 2.0,
            width: 18.0 * s,
            height: 18.0 * s,
        }),
        _ => None,
    }
}

pub fn draw_icon(bitmap: &mut Pixmap, tile: &FrameTile) {
    let Some(icon) = tile.style.and_then(|style| style.role.home_icon()) else {
        return;
    };
    let Some(rect) = icon_rect(tile) else { return };
    let mut p = PathBuilder::new();
    // All symbols share a 24-unit grid and the same stroke and rounded ends.
    match icon {
        HomeIcon::Point => {
            p.push_circle(12.0, 12.0, 6.0);
            segments(
                &mut p,
                &[
                    (12., 2., 12., 8.),
                    (12., 16., 12., 22.),
                    (2., 12., 8., 12.),
                    (16., 12., 22., 12.),
                ],
            );
        }
        HomeIcon::Mouse => {
            p.move_to(6., 9.);
            p.cubic_to(6., 1., 18., 1., 18., 9.);
            p.line_to(18., 15.);
            p.cubic_to(18., 23., 6., 23., 6., 15.);
            p.close();
            segments(&mut p, &[(6., 10., 18., 10.), (12., 3., 12., 10.)]);
        }
        HomeIcon::Keyboard => {
            rect_path(&mut p, 2., 5., 20., 14.);
            for y in [9., 12.] {
                for x in [6., 10., 14., 18.] {
                    segments(&mut p, &[(x, y, x + 0.2, y)]);
                }
            }
            segments(&mut p, &[(7., 16., 17., 16.)]);
        }
        HomeIcon::Windows => {
            rect_path(&mut p, 3., 7., 14., 14.);
            p.move_to(7., 7.);
            p.line_to(7., 3.);
            p.line_to(21., 3.);
            p.line_to(21., 17.);
            p.line_to(17., 17.);
            segments(&mut p, &[(3., 11., 17., 11.)]);
        }
        HomeIcon::Editing => {
            p.move_to(4., 16.);
            p.line_to(15., 5.);
            p.line_to(19., 9.);
            p.line_to(8., 20.);
            p.line_to(3., 21.);
            p.close();
            segments(&mut p, &[(13., 7., 17., 11.)]);
        }
        HomeIcon::Browser => {
            rect_path(&mut p, 3., 4., 18., 16.);
            segments(
                &mut p,
                &[
                    (3., 9., 21., 9.),
                    (7., 6.5, 7.2, 6.5),
                    (10., 6.5, 10.2, 6.5),
                ],
            );
        }
        HomeIcon::Media => {
            p.move_to(8., 4.);
            p.line_to(20., 12.);
            p.line_to(8., 20.);
            p.close();
        }
        HomeIcon::Zoom => {
            p.push_circle(10., 10., 6.);
            segments(
                &mut p,
                &[
                    (14.5, 14.5, 21., 21.),
                    (7., 10., 13., 10.),
                    (10., 7., 10., 13.),
                ],
            );
        }
        HomeIcon::Settings => {
            segments(
                &mut p,
                &[
                    (3., 6., 7., 6.),
                    (11., 6., 21., 6.),
                    (3., 12., 13., 12.),
                    (17., 12., 21., 12.),
                    (3., 18., 7., 18.),
                    (11., 18., 21., 18.),
                ],
            );
            p.push_circle(9., 6., 2.);
            p.push_circle(15., 12., 2.);
            p.push_circle(9., 18., 2.);
        }
        HomeIcon::Panels => {
            for (x, y) in [(3., 3.), (14., 3.), (3., 14.), (14., 14.)] {
                rect_path(&mut p, x, y, 7., 7.);
            }
        }
        HomeIcon::Back => {
            segments(
                &mut p,
                &[(20., 12., 4., 12.), (4., 12., 11., 5.), (4., 12., 11., 19.)],
            );
        }
        HomeIcon::Close => {
            segments(&mut p, &[(5., 5., 19., 19.), (5., 19., 19., 5.)]);
        }
        HomeIcon::Action => {
            p.move_to(13., 2.);
            p.line_to(4., 14.);
            p.line_to(11., 14.);
            p.line_to(10., 22.);
            p.line_to(20., 10.);
            p.line_to(13., 10.);
            p.close();
        }
    }
    let Some(path) = p.finish() else { return };
    let [r, g, b] = tile.appearance.palette().text;
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(
        r,
        g,
        b,
        if tile.selected { 255 } else { 200 },
    ));
    bitmap.stroke_path(
        &path,
        &paint,
        &Stroke {
            width: 1.8,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        },
        Transform::from_scale(rect.width as f32 / 24., rect.height as f32 / 24.)
            .post_translate(rect.x as f32, rect.y as f32),
        None,
    );
}

fn segments(p: &mut PathBuilder, lines: &[(f32, f32, f32, f32)]) {
    for &(x1, y1, x2, y2) in lines {
        p.move_to(x1, y1);
        p.line_to(x2, y2);
    }
}
fn rect_path(p: &mut PathBuilder, x: f32, y: f32, width: f32, height: f32) {
    p.move_to(x, y);
    p.line_to(x + width, y);
    p.line_to(x + width, y + height);
    p.line_to(x, y + height);
    p.close();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        panel_theme::Appearance,
        scanning::{ScannerColor, TileStyle},
    };

    #[test]
    fn icons_and_labels_have_separate_bounds_at_every_panel_scale() {
        for scale in [0.25, 0.75, 1.0, 1.5, 2.0] {
            for navigation in [false, true] {
                for appearance in [Appearance::Light, Appearance::Dark] {
                    for color in [
                        ScannerColor::Red,
                        ScannerColor::Green,
                        ScannerColor::Blue,
                        ScannerColor::Yellow,
                        ScannerColor::White,
                    ] {
                        for icon in [
                            HomeIcon::Point,
                            HomeIcon::Mouse,
                            HomeIcon::Keyboard,
                            HomeIcon::Windows,
                            HomeIcon::Editing,
                            HomeIcon::Browser,
                            HomeIcon::Media,
                            HomeIcon::Zoom,
                            HomeIcon::Settings,
                            HomeIcon::Panels,
                            HomeIcon::Back,
                            HomeIcon::Close,
                            HomeIcon::Action,
                        ] {
                            let tile = FrameTile {
                                rect: Rect {
                                    x: 0.,
                                    y: 0.,
                                    width: 176. * scale,
                                    height: if navigation { 64. } else { 88. } * scale,
                                },
                                scale,
                                color,
                                appearance,
                                text: "Label".into(),
                                thickness: Default::default(),
                                style: Some(TileStyle {
                                    role: if navigation {
                                        TileRole::HomeNavigation(icon)
                                    } else {
                                        TileRole::HomeAction(icon)
                                    },
                                    active: false,
                                    row_scan: false,
                                }),
                                selected: true,
                            };
                            let label = crate::scan_tile::label_rect(&tile);
                            let icon = icon_rect(&tile).unwrap();
                            assert!(label.width > 0. && label.height > 0.);
                            assert!(icon.x >= 0. && icon.y >= 0.);
                            assert!(label.x + label.width <= tile.rect.width + 0.01);
                            assert!(label.y + label.height <= tile.rect.height + 0.01);
                            assert!(if navigation {
                                icon.x + icon.width < label.x
                            } else {
                                icon.y + icon.height < label.y
                            });
                            // Rendering uses no native input or platform fonts.
                            let mut without = crate::scan_tile::bitmap(&FrameTile {
                                style: Some(TileStyle {
                                    role: TileRole::Character,
                                    active: false,
                                    row_scan: false,
                                }),
                                ..tile.clone()
                            })
                            .unwrap();
                            let original = without.clone();
                            draw_icon(&mut without, &tile);
                            assert_ne!(without, original);
                        }
                    }
                }
            }
        }
    }
}
