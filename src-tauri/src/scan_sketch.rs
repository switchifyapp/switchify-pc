//! Smooth lines, rings and outlines for scan overlays, such as Radar's beam.
//! Native overlays only show rectangles and bitmaps, so a drawing is cut into
//! square cells that never overlap. Only cells something passes through are
//! drawn, each as its own small bitmap, so a turning beam never repaints the
//! whole display.
use crate::scanning::Rect;
use tiny_skia::{
    FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

/// The dark edge every mark carries, so it shows on light and dark windows.
const EDGE: [u8; 3] = [10, 12, 18];
/// Device pixels drawn beyond each side of a cell and then cropped away.
const MARGIN: u32 = 4;
/// Length of the near-invisible dash that round caps turn into a dot.
const DOT_ON: f32 = 0.01;

/// Distance from one dot to the next on a dotted line of solid-line width
/// `width`. A dotted line starts with a dot, so a caller can keep its dots
/// still while its start moves by starting it on a whole number of spacings.
pub fn dot_spacing(width: f64) -> f64 {
    width * 2.0 + f64::from(DOT_ON)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A solid line in the accent colour.
    Line {
        from: (f64, f64),
        to: (f64, f64),
    },
    /// A faint dotted line, for where a line can still go.
    Dotted {
        from: (f64, f64),
        to: (f64, f64),
    },
    /// A dashed light line dividing the display into areas.
    Divider {
        from: (f64, f64),
        to: (f64, f64),
    },
    Ring {
        centre: (f64, f64),
        radius: f64,
    },
    Dot {
        centre: (f64, f64),
        radius: f64,
    },
    /// A rounded rectangle drawn as an outline.
    Outline {
        rect: Rect,
        radius: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sketch {
    pub shapes: Vec<Shape>,
    pub color: [u8; 3],
    /// Width of a solid line in display units. Other marks scale with it.
    pub width: f64,
    /// Nothing is drawn outside this, normally the display.
    pub bounds: Rect,
    /// Cells are this many display units square, counted from the top left
    /// of `bounds`.
    pub cell: f64,
}

/// How a shape is stroked: its path, the colour of its middle, and the
/// widths and dash of its middle and its dark edge.
struct Style {
    path: Path,
    color: [u8; 3],
    alpha: u8,
    width: f32,
    edge_alpha: u8,
    edge_width: f32,
    dash: Option<(f32, f32)>,
    fill: bool,
}

fn segment((a, b): (f64, f64), (c, d): (f64, f64)) -> Option<Path> {
    let mut path = PathBuilder::new();
    path.move_to(a as f32, b as f32);
    path.line_to(c as f32, d as f32);
    path.finish()
}

fn circle((x, y): (f64, f64), radius: f64) -> Option<Path> {
    PathBuilder::from_circle(x as f32, y as f32, radius.max(0.5) as f32)
}

fn rounded(rect: Rect, radius: f64) -> Option<Path> {
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    // A cubic of this weight is the usual close match to a quarter circle.
    let k = r * 0.552_284_75;
    let mut p = PathBuilder::new();
    let f = |v: f64| v as f32;
    p.move_to(f(x + r), f(y));
    p.line_to(f(x + w - r), f(y));
    p.cubic_to(
        f(x + w - r + k),
        f(y),
        f(x + w),
        f(y + r - k),
        f(x + w),
        f(y + r),
    );
    p.line_to(f(x + w), f(y + h - r));
    p.cubic_to(
        f(x + w),
        f(y + h - r + k),
        f(x + w - r + k),
        f(y + h),
        f(x + w - r),
        f(y + h),
    );
    p.line_to(f(x + r), f(y + h));
    p.cubic_to(
        f(x + r - k),
        f(y + h),
        f(x),
        f(y + h - r + k),
        f(x),
        f(y + h - r),
    );
    p.line_to(f(x), f(y + r));
    p.cubic_to(f(x), f(y + r - k), f(x + r - k), f(y), f(x + r), f(y));
    p.close();
    p.finish()
}

/// Whether the segment from `a` to `b` passes through `rect` (Liang–Barsky).
fn crosses(rect: Rect, a: (f64, f64), b: (f64, f64)) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.0 - rect.x),
        (dx, rect.x + rect.width - a.0),
        (-dy, a.1 - rect.y),
        (dy, rect.y + rect.height - a.1),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                low = low.max(t);
            } else {
                high = high.min(t);
            }
            if low > high {
                return false;
            }
        }
    }
    true
}

fn grow(rect: Rect, by: f64) -> Rect {
    Rect {
        x: rect.x - by,
        y: rect.y - by,
        width: rect.width + by * 2.0,
        height: rect.height + by * 2.0,
    }
}

impl Shape {
    /// Whether any ink of this shape can land in `cell`, given that ink
    /// spreads up to `pad` from its lines or circle.
    fn touches(&self, cell: Rect, pad: f64) -> bool {
        let cell = grow(cell, pad);
        match *self {
            Self::Line { from, to } | Self::Dotted { from, to } | Self::Divider { from, to } => {
                crosses(cell, from, to)
            }
            Self::Ring { centre, radius } | Self::Dot { centre, radius } => {
                crosses(grow(cell, radius), centre, centre)
            }
            Self::Outline { rect, .. } => {
                let (l, t) = (rect.x, rect.y);
                let (r, b) = (rect.x + rect.width, rect.y + rect.height);
                [
                    ((l, t), (r, t)),
                    ((r, t), (r, b)),
                    ((r, b), (l, b)),
                    ((l, b), (l, t)),
                ]
                .iter()
                .any(|&(a, z)| crosses(cell, a, z))
            }
        }
    }
    fn style(&self, color: [u8; 3], w: f64) -> Option<Style> {
        let w = w as f32;
        let solid = |path, width: f32| Style {
            path,
            color,
            alpha: 255,
            width,
            edge_alpha: 217,
            edge_width: width + w * 0.75,
            dash: None,
            fill: false,
        };
        Some(match *self {
            Self::Line { from, to } => solid(segment(from, to)?, w),
            Self::Ring { centre, radius } => solid(circle(centre, radius)?, w * 0.75),
            Self::Outline { rect, radius } => solid(rounded(rect, radius)?, w),
            Self::Dot { centre, radius } => Style {
                fill: true,
                ..solid(circle(centre, radius)?, w * 0.5)
            },
            Self::Dotted { from, to } => Style {
                alpha: 210,
                edge_alpha: 190,
                // A dash too short to see, with round caps, is a dot.
                dash: Some((DOT_ON, dot_spacing(f64::from(w)) as f32 - DOT_ON)),
                edge_width: w,
                ..solid(segment(from, to)?, w * 0.6)
            },
            Self::Divider { from, to } => Style {
                color: [255, 255, 255],
                alpha: 210,
                edge_alpha: 150,
                dash: Some((w * 1.5, w * 1.5)),
                ..solid(segment(from, to)?, (w * 0.35).max(1.5))
            },
        })
    }
}

impl Sketch {
    /// How far ink can spread from a shape's lines, centre or edge.
    fn pad(&self) -> f64 {
        self.width * 1.5 + 2.0
    }
    fn cell_size(&self) -> f64 {
        self.cell.round().max(16.0)
    }
    /// The cells to show, in reading order. Each lies inside `bounds`, and
    /// no two overlap.
    pub fn cells(&self) -> Vec<Rect> {
        let b = self.bounds;
        let size = self.cell_size();
        let columns = (b.width / size).ceil().max(0.0) as usize;
        let rows = (b.height / size).ceil().max(0.0) as usize;
        let pad = self.pad();
        let mut cells = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                let x = b.x + column as f64 * size;
                let y = b.y + row as f64 * size;
                let cell = Rect {
                    x,
                    y,
                    width: size.min(b.x + b.width - x),
                    height: size.min(b.y + b.height - y),
                };
                if self.shapes.iter().any(|shape| shape.touches(cell, pad)) {
                    cells.push(cell);
                }
            }
        }
        cells
    }
    /// The shapes whose ink can land in `cell`. A cell whose shapes, colour,
    /// width and bounds are unchanged draws the same pixels.
    pub fn touching(&self, cell: Rect) -> Vec<Shape> {
        let pad = self.pad();
        self.shapes
            .iter()
            .filter(|shape| shape.touches(cell, pad))
            .copied()
            .collect()
    }
    /// Whether `cell` draws the same pixels in this sketch as in `other`.
    pub fn draws_alike(&self, other: &Self, cell: Rect) -> bool {
        self.color == other.color
            && self.width == other.width
            && self.bounds == other.bounds
            && self.cell == other.cell
            && self.touching(cell) == other.touching(cell)
    }
    /// One cell's pixels, premultiplied RGBA, at `ratio` device pixels per
    /// display unit.
    pub fn draw(&self, cell: Rect, ratio: f64) -> Result<Pixmap, String> {
        let ratio = if ratio.is_finite() && ratio > 0.0 {
            ratio
        } else {
            1.0
        };
        let width = (cell.width * ratio).round().clamp(1.0, 4096.0) as u32;
        let height = (cell.height * ratio).round().clamp(1.0, 4096.0) as u32;
        // Drawn with a margin and cropped, so anti-aliasing at the edges of a
        // cell matches its neighbours rather than being cut by the bitmap edge.
        let mut pixmap = Pixmap::new(width + MARGIN * 2, height + MARGIN * 2)
            .ok_or("Cannot draw the scanning overlay.")?;
        let transform = Transform::from_translate(MARGIN as f32, MARGIN as f32)
            .pre_scale(ratio as f32, ratio as f32)
            .pre_translate(-cell.x as f32, -cell.y as f32);
        let styles: Vec<Style> = self
            .touching(cell)
            .iter()
            .filter_map(|shape| shape.style(self.color, self.width))
            .collect();
        // Every dark edge goes down first, so a line meeting a ring reads as
        // one mark rather than one cut by the other's edge.
        for edge in [true, false] {
            for style in &styles {
                let (color, alpha) = if edge {
                    (EDGE, style.edge_alpha)
                } else {
                    (style.color, style.alpha)
                };
                let mut paint = Paint::default();
                paint.set_color_rgba8(color[0], color[1], color[2], alpha);
                let width = if edge { style.edge_width } else { 0.0 };
                if style.fill {
                    if edge {
                        let stroke = Stroke {
                            width,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&style.path, &paint, &stroke, transform, None);
                    }
                    pixmap.fill_path(&style.path, &paint, FillRule::Winding, transform, None);
                    continue;
                }
                let stroke = Stroke {
                    width: if edge { width } else { style.width },
                    line_cap: LineCap::Round,
                    dash: style
                        .dash
                        .and_then(|(on, off)| StrokeDash::new(vec![on, off], 0.0)),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&style.path, &paint, &stroke, transform, None);
            }
        }
        pixmap
            .clone_rect(
                tiny_skia::IntRect::from_xywh(MARGIN as i32, MARGIN as i32, width, height)
                    .ok_or("Cannot draw the scanning overlay.")?,
            )
            .ok_or_else(|| "Cannot draw the scanning overlay.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Rect = Rect {
        x: -1000.0,
        y: 100.0,
        width: 1000.0,
        height: 800.0,
    };

    fn sketch(shapes: Vec<Shape>) -> Sketch {
        Sketch {
            shapes,
            color: [100, 166, 255],
            width: 4.0,
            bounds: DISPLAY,
            cell: 100.0,
        }
    }

    #[test]
    fn only_cells_a_moving_line_passes_through_draw_differently() {
        let side = Shape::Divider {
            from: (DISPLAY.x, DISPLAY.y + 50.0),
            to: (DISPLAY.x + DISPLAY.width, DISPLAY.y + 50.0),
        };
        let beam = |to: (f64, f64)| Shape::Line {
            from: (DISPLAY.x + 50.0, DISPLAY.y + 350.0),
            to,
        };
        let before = sketch(vec![side, beam((DISPLAY.x + 50.0, DISPLAY.y + 250.0))]);
        let after = sketch(vec![side, beam((DISPLAY.x + 50.0, DISPLAY.y + 260.0))]);
        let cell = |column: f64, row: f64| Rect {
            x: DISPLAY.x + column * 100.0,
            y: DISPLAY.y + row * 100.0,
            width: 100.0,
            height: 100.0,
        };
        assert!(after.draws_alike(&before, cell(3.0, 0.0)), "the side alone");
        assert!(
            !after.draws_alike(&before, cell(0.0, 2.0)),
            "the beam's tip"
        );
        let recoloured = Sketch {
            color: [255, 0, 0],
            ..before.clone()
        };
        assert!(!recoloured.draws_alike(&before, cell(3.0, 0.0)));
        for c in after.cells() {
            if after.draws_alike(&before, c) {
                assert_eq!(after.draw(c, 2.0), before.draw(c, 2.0));
            }
        }
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    fn alpha_at(sketch: &Sketch, (x, y): (f64, f64)) -> u8 {
        // A point no cell covers is not drawn on at all.
        let Some(cell) = sketch
            .cells()
            .into_iter()
            .find(|c| x >= c.x && x < c.x + c.width && y >= c.y && y < c.y + c.height)
        else {
            return 0;
        };
        let pixmap = sketch.draw(cell, 1.0).unwrap();
        pixmap
            .pixel((x - cell.x) as u32, (y - cell.y) as u32)
            .unwrap()
            .alpha()
    }

    #[test]
    fn a_line_uses_only_the_cells_it_crosses() {
        let s = sketch(vec![Shape::Line {
            from: (-950.0, 150.0),
            to: (-50.0, 150.0),
        }]);
        let cells = s.cells();
        assert_eq!(cells.len(), 10, "one row of a 10-cell-wide display");
        assert!(cells.iter().all(|c| c.y == 100.0 && c.height == 100.0));
        for (i, a) in cells.iter().enumerate() {
            assert!(cells[i + 1..].iter().all(|b| !overlaps(*a, *b)));
        }
    }

    #[test]
    fn cells_stay_inside_the_bounds() {
        let s = Sketch {
            bounds: Rect {
                width: 950.0,
                height: 777.0,
                ..DISPLAY
            },
            ..sketch(vec![Shape::Line {
                from: (-1200.0, 0.0),
                to: (200.0, 1200.0),
            }])
        };
        for c in s.cells() {
            assert!(c.x >= s.bounds.x && c.y >= s.bounds.y);
            assert!(c.x + c.width <= s.bounds.x + s.bounds.width + 1e-9);
            assert!(c.y + c.height <= s.bounds.y + s.bounds.height + 1e-9);
        }
    }

    #[test]
    fn a_line_is_drawn_with_a_coloured_middle_and_a_dark_edge() {
        let s = sketch(vec![Shape::Line {
            from: (-900.0, 450.0),
            to: (-100.0, 450.0),
        }]);
        let cell = s.cells()[3];
        let p = s.draw(cell, 1.0).unwrap();
        let middle = p.pixel(50, (450.0 - cell.y) as u32).unwrap();
        assert_eq!(middle.alpha(), 255);
        assert!(middle.blue() > middle.red(), "accent colour in the middle");
        let edge = p.pixel(50, (450.0 - cell.y + 2.6) as u32).unwrap();
        assert!(
            edge.alpha() > 150 && edge.blue() < 60,
            "dark edge outside it"
        );
        assert_eq!(p.pixel(50, 0).unwrap().alpha(), 0, "clear elsewhere");
    }

    #[test]
    fn a_dotted_line_is_fainter_and_has_gaps() {
        let s = sketch(vec![Shape::Dotted {
            from: (-900.0, 450.0),
            to: (-100.0, 450.0),
        }]);
        let cell = s.cells()[3];
        let p = s.draw(cell, 1.0).unwrap();
        let row: Vec<u8> = (0..100)
            .map(|x| p.pixel(x, (450.0 - cell.y) as u32).unwrap().alpha())
            .collect();
        assert!(row.contains(&0), "gaps between dots");
        assert!(row.iter().any(|&a| a > 0), "dots");
    }

    #[test]
    fn neighbouring_cells_join_without_a_seam() {
        let s = sketch(vec![Shape::Line {
            from: (-1000.0, 450.0),
            to: (0.0, 450.0),
        }]);
        assert_eq!(alpha_at(&s, (-901.0, 450.0)), 255);
        assert_eq!(alpha_at(&s, (-900.0, 450.0)), 255);
    }

    #[test]
    fn a_ring_shows_in_every_cell_it_reaches() {
        // Centred on a cell corner, so it spans four cells.
        let s = sketch(vec![Shape::Ring {
            centre: (-500.0, 500.0),
            radius: 9.0,
        }]);
        assert_eq!(s.cells().len(), 4);
        assert!(alpha_at(&s, (-491.0, 500.0)) > 200);
        assert!(alpha_at(&s, (-509.0, 500.0)) > 200);
        assert_eq!(alpha_at(&s, (-500.0, 500.0)), 0, "hollow");
    }

    #[test]
    fn a_rounded_outline_draws_its_edges_and_leaves_the_inside_clear() {
        let s = sketch(vec![Shape::Outline {
            rect: Rect {
                x: -980.0,
                y: 120.0,
                width: 460.0,
                height: 360.0,
            },
            radius: 30.0,
        }]);
        assert_eq!(alpha_at(&s, (-750.0, 120.0)), 255);
        assert_eq!(alpha_at(&s, (-750.0, 300.0)), 0);
        assert_eq!(
            alpha_at(&s, (-980.0, 120.0)),
            0,
            "the corner is rounded off"
        );
    }

    #[test]
    fn higher_density_draws_more_pixels_for_the_same_cell() {
        let s = sketch(vec![Shape::Dot {
            centre: (-950.0, 150.0),
            radius: 5.0,
        }]);
        let cell = s.cells()[0];
        let p = s.draw(cell, 2.0).unwrap();
        assert_eq!((p.width(), p.height()), (200, 200));
        assert_eq!(p.pixel(100, 100).unwrap().alpha(), 255);
    }

    #[test]
    fn stitched_cells_match_one_drawing_of_the_whole_display() {
        // Every kind of mark, thick, with a divider exactly on a cell edge
        // and marks running off the display.
        let s = Sketch {
            width: 18.0,
            ..sketch(vec![
                Shape::Divider {
                    from: (-500.0, 100.0),
                    to: (-500.0, 900.0),
                },
                Shape::Divider {
                    from: (-1000.0, 500.0),
                    to: (0.0, 500.0),
                },
                Shape::Outline {
                    rect: Rect {
                        x: -973.0,
                        y: 127.0,
                        width: 446.0,
                        height: 346.0,
                    },
                    radius: 36.0,
                },
                Shape::Line {
                    from: (-250.0, 700.0),
                    to: (-1100.0, 0.0),
                },
                Shape::Dotted {
                    from: (-250.0, 700.0),
                    to: (60.0, 950.0),
                },
                Shape::Ring {
                    centre: (-600.0, 300.0),
                    radius: 40.0,
                },
                Shape::Dot {
                    centre: (-1.0, 101.0),
                    radius: 22.0,
                },
            ])
        };
        let whole = s.draw(s.bounds, 1.0).unwrap();
        let mut stitched = Pixmap::new(whole.width(), whole.height()).unwrap();
        for cell in s.cells() {
            let part = s.draw(cell, 1.0).unwrap();
            stitched.draw_pixmap(
                (cell.x - s.bounds.x) as i32,
                (cell.y - s.bounds.y) as i32,
                part.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        // Anti-aliased edges differ by a few levels wherever the rasteriser's
        // origin moves, inside a cell as much as at its edge. A clipped or
        // missing piece would differ by far more.
        let worst = whole
            .data()
            .iter()
            .zip(stitched.data())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(worst <= 24, "cells differ from the whole by up to {worst}");
        assert!(whole.data().iter().any(|&byte| byte > 0));
    }

    #[test]
    fn nothing_to_draw_needs_no_cells() {
        assert!(sketch(vec![]).cells().is_empty());
    }
}
