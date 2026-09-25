//! Shared layout for switch-scanned panels: a docked background, a status tile
//! and weighted rows of keys. Panels supply content; geometry lives here.
use crate::{
    scan_menu::Item,
    scan_preferences::Thickness,
    scanning::{Frame, FrameTile, Rect, ScannerColor, TileRole, TileStyle},
};

pub struct PanelKey {
    pub text: String,
    pub weight: f64,
    pub role: TileRole,
    pub active: bool,
    /// Reserves a cell without drawing it or counting it as a scanner column.
    pub blank: bool,
}

impl PanelKey {
    pub fn blank() -> Self {
        Self {
            text: String::new(),
            weight: 1.0,
            role: TileRole::Utility,
            active: false,
            blank: true,
        }
    }
}

/// One of nine screen positions. `column` and `row` are 0 (start), 1 (middle) or 2 (end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dock {
    pub column: u8,
    pub row: u8,
}

impl Default for Dock {
    fn default() -> Self {
        Self { column: 1, row: 2 }
    }
}

impl Dock {
    pub fn label(self) -> &'static str {
        const LABELS: [[&str; 3]; 3] = [
            ["Top left", "Top", "Top right"],
            ["Left", "Middle", "Right"],
            ["Bottom left", "Bottom", "Bottom right"],
        ];
        LABELS[usize::from(self.row.min(2))][usize::from(self.column.min(2))]
    }
    /// Start of a `size` span placed at `slot` within `length` from `start`.
    fn place(slot: u8, start: f64, length: f64, size: f64) -> f64 {
        start + (length - size) * f64::from(slot.min(2)) / 2.0
    }
    /// Outer rectangle of a panel docked here.
    pub fn rect(self, screen: Rect, units: f64) -> Rect {
        let width = (1180.0 * units).min((screen.width - 24.0 * units).max(1.0));
        let height = (460.0 * units).min(screen.height * 0.55).max(1.0);
        Rect {
            x: Self::place(self.column, screen.x, screen.width, width),
            y: Self::place(self.row, screen.y, screen.height, height),
            width,
            height,
        }
    }
    /// Where to draw a panel docked here while the pointer may be over it. The panel moves to
    /// an end of the screen the pointer is not over, until the pointer leaves this dock. A
    /// middle panel keeps its previous end (`moved`) while that end stays clear of the pointer.
    /// On short screens both ends can cover the pointer; the panel then stays put.
    pub fn avoiding(
        self,
        pointer: (f64, f64),
        screen: Rect,
        units: f64,
        moved: Option<Dock>,
    ) -> Option<Dock> {
        let covers = |dock: Dock| {
            let rect = dock.rect(screen, units);
            let (x, y) = pointer;
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        };
        if !covers(self) {
            return None;
        }
        let rect = self.rect(screen, units);
        let away = if pointer.1 < rect.y + rect.height / 2.0 {
            2
        } else {
            0
        };
        let rows = match (self.row, moved) {
            (0, _) => [2, 2],
            (2, _) => [0, 0],
            (_, Some(moved)) if moved.column == self.column && moved.row != 1 => {
                [moved.row, 2 - moved.row]
            }
            _ => [away, 2 - away],
        };
        rows.into_iter()
            .map(|row| Dock { row, ..self })
            .find(|&dock| !covers(dock))
    }
}

/// Shifts a panel frame drawn at `from` so it is drawn at `to` instead.
pub fn move_frame(frame: &mut Frame, from: Dock, to: Dock, screen: Rect, units: f64) {
    let (from, to) = (from.rect(screen, units), to.rect(screen, units));
    for tile in &mut frame.tiles {
        tile.rect.x += to.x - from.x;
        tile.rect.y += to.y - from.y;
    }
}

/// Position page shared by panels: the nine docks as a 3×3 grid, then a back key.
pub const POSITION_PAGE: &str = "Position";
pub fn position_rows<K>(position: impl Fn(Dock) -> K, back: K) -> Vec<Vec<K>> {
    let mut rows: Vec<Vec<K>> = (0..3)
        .map(|row| {
            (0..3)
                .map(|column| position(Dock { column, row }))
                .collect()
        })
        .collect();
    rows.push(vec![back]);
    rows
}
pub fn position_label(dock: Dock, current: Dock) -> String {
    format!(
        "{}{}",
        dock.label(),
        if dock == current { " •" } else { "" }
    )
}

pub struct Panel {
    pub rows: Vec<Vec<PanelKey>>,
    pub status: String,
    /// Passive feedback beside the scanning prompt; never a scan target.
    pub note: Option<String>,
    pub dock: Dock,
    /// Scanner position to highlight, or `None` when no key may be chosen.
    pub selected: Option<(usize, Option<usize>)>,
    /// Highlights the status tile, used while the scan offers to go back to rows.
    pub status_selected: bool,
    pub row_scan: bool,
    pub thickness: Thickness,
}

impl Panel {
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let Rect {
            x,
            y,
            width,
            height,
        } = self.dock.rect(screen, units);
        let outer_scale = units.min(height / 460.0).min(width / 1180.0);
        let padding = 16.0 * outer_scale;
        let content_width = (width - padding * 2.0).max(1.0);
        let content_height = (height - padding * 2.0).max(1.0);
        let row_height = content_height / (self.rows.len() as f64 + 0.9);
        let scale = units.min(row_height / 60.0).min(content_width / 1040.0);
        let gap = (6.0 * scale).min(row_height / 8.0);
        let header_height = row_height * 0.9;
        let mut frame = Frame::default();
        frame.tiles.push(FrameTile::panel_background(
            Rect {
                x,
                y,
                width,
                height,
            },
            outer_scale,
            color,
        ));
        let x = x + padding;
        let y = y + padding;
        let width = content_width;
        for (r, row) in self.rows.iter().enumerate() {
            let total_weight: f64 = row.iter().map(|key| key.weight).sum();
            let cell_unit = (width - gap * (row.len() - 1) as f64).max(1.0) / total_weight;
            let mut left = x;
            let mut c = 0;
            for key in row {
                let key_width = cell_unit * key.weight;
                if key.blank {
                    left += key_width + gap;
                    continue;
                }
                frame.tiles.push(FrameTile {
                    thickness: self.thickness,
                    color,
                    text: key.text.clone(),
                    icon: Item::KeyboardKey,
                    style: Some(TileStyle {
                        role: key.role,
                        active: key.active,
                        row_scan: self.row_scan,
                    }),
                    rect: Rect {
                        x: left,
                        y: y + header_height + r as f64 * row_height,
                        width: key_width,
                        height: (row_height - gap).max(1.0),
                    },
                    scale,
                    selected: self.selected.is_some_and(|(active_row, active_column)| {
                        r == active_row && active_column.is_none_or(|column| c == column)
                    }),
                });
                left += key_width + gap;
                c += 1;
            }
        }
        let note_width = self.note.as_ref().map_or(0.0, |_| width * 0.28);
        if let Some(note) = &self.note {
            frame.tiles.push(FrameTile {
                thickness: self.thickness,
                color,
                text: note.clone(),
                icon: Item::KeyboardKey,
                style: Some(TileStyle {
                    role: TileRole::Status,
                    active: false,
                    row_scan: false,
                }),
                rect: Rect {
                    x: x + width - note_width,
                    y,
                    width: note_width,
                    height: (header_height - gap).max(1.0),
                },
                scale: scale * 0.8,
                selected: false,
            });
        }
        frame.tiles.push(FrameTile {
            thickness: self.thickness,
            color,
            text: self.status.clone(),
            icon: Item::KeyboardKey,
            style: Some(TileStyle {
                role: TileRole::Status,
                active: false,
                row_scan: false,
            }),
            rect: Rect {
                x,
                y,
                width: width - note_width - if note_width > 0.0 { gap } else { 0.0 },
                height: (header_height - gap).max(1.0),
            },
            scale,
            selected: self.status_selected,
        });
        frame
    }
}

/// Status wording shared by panels once error and pause states are handled.
pub fn scanning_status(page: &str, row_scan: bool, key: &str) -> String {
    if row_scan {
        format!("{page} · Select a row")
    } else {
        format!("{page} · Select {}", key.replace('\n', " "))
    }
}
pub const BACK_TO_ROWS: &str = "Back to rows · Select to return";

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str, weight: f64) -> PanelKey {
        PanelKey {
            text: text.into(),
            weight,
            role: TileRole::Utility,
            active: false,
            blank: false,
        }
    }

    #[test]
    fn rows_fill_the_padded_width_with_even_gaps_below_the_status_tile() {
        let panel = Panel {
            rows: vec![vec![key("a", 1.0), key("b", 2.0)], vec![key("c", 1.0)]],
            status: "Page · Select a row".into(),
            note: None,
            dock: Dock::default(),
            selected: Some((1, None)),
            status_selected: false,
            row_scan: true,
            thickness: Thickness::Thick,
        };
        let screen = Rect {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default());
        let [background, a, b, c, status] = frame.tiles.as_slice() else {
            panic!("expected background, three keys and a status tile");
        };
        assert!(background.is_panel_background());
        assert_eq!(background.rect.width, 1180.0);
        assert_eq!(background.rect.y + background.rect.height, 1080.0);
        let (left, right) = (background.rect.x + 16.0, background.rect.x + 1180.0 - 16.0);
        assert_eq!(a.rect.x, left);
        assert!((b.rect.x + b.rect.width - right).abs() < 1e-9);
        assert!((c.rect.x + c.rect.width - right).abs() < 1e-9);
        assert!((b.rect.width - 2.0 * a.rect.width).abs() < 1e-9);
        assert!((b.rect.x - (a.rect.x + a.rect.width) - 6.0).abs() < 1e-9);
        assert_eq!(status.style.unwrap().role, TileRole::Status);
        assert_eq!(
            (status.rect.x, status.rect.y),
            (left, background.rect.y + 16.0)
        );
        assert!(status.rect.y + status.rect.height < a.rect.y);
        assert_eq!(
            frame.tiles.iter().filter(|tile| tile.selected).count(),
            1,
            "row scan highlights only row 1"
        );
        assert!(c.selected && c.thickness == Thickness::Thick);
    }

    #[test]
    fn docking_and_status_selection() {
        let panel = Panel {
            rows: vec![vec![key("a", 1.0)]],
            status: BACK_TO_ROWS.into(),
            note: None,
            dock: Dock { column: 1, row: 0 },
            selected: None,
            status_selected: true,
            row_scan: false,
            thickness: Thickness::default(),
        };
        let screen = Rect {
            x: 0.0,
            y: 50.0,
            width: 800.0,
            height: 400.0,
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default());
        assert_eq!(frame.tiles[0].rect.y, 50.0);
        assert!(frame.tiles[0].rect.x >= 0.0 && frame.tiles[0].rect.width <= 800.0);
        assert!(!frame.tiles[1].selected);
        assert!(frame.tiles[2].selected);
        assert_eq!(
            scanning_status("Movement", false, "Caps lock\nOn"),
            "Movement · Select Caps lock On"
        );
        assert_eq!(
            scanning_status("Movement", true, "x"),
            "Movement · Select a row"
        );
    }

    #[test]
    fn avoiding_the_pointer_moves_to_the_other_end_until_it_leaves_the_dock() {
        let screen = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        let dock = |column, row| Dock { column, row };
        for column in 0..3 {
            let bottom = dock(column, 2).rect(screen, 1.0);
            let inside = (bottom.x + 1.0, bottom.y + bottom.height - 1.0);
            let outside = (bottom.x + 1.0, bottom.y - 1.0);
            assert_eq!(
                dock(column, 2).avoiding(inside, screen, 1.0, None),
                Some(dock(column, 0))
            );
            assert_eq!(dock(column, 2).avoiding(outside, screen, 1.0, None), None);
            let top = dock(column, 0).rect(screen, 1.0);
            assert_eq!(
                dock(column, 0).avoiding((top.x, top.y), screen, 1.0, None),
                Some(dock(column, 2))
            );
        }
        let middle = dock(1, 1).rect(screen, 1.0);
        let upper = (middle.x + 10.0, middle.y + 10.0);
        let lower = (middle.x + 10.0, middle.y + middle.height - 10.0);
        assert_eq!(
            dock(1, 1).avoiding(upper, screen, 1.0, None),
            Some(dock(1, 2))
        );
        assert_eq!(
            dock(1, 1).avoiding(lower, screen, 1.0, None),
            Some(dock(1, 0))
        );
        let centre = (middle.x + 10.0, middle.y + middle.height / 2.0 + 1.0);
        assert_eq!(
            dock(1, 1).avoiding(centre, screen, 1.0, Some(dock(1, 2))),
            Some(dock(1, 2)),
            "a moved middle panel stays put while its end is clear of the pointer"
        );
        assert_eq!(
            dock(1, 1).avoiding(lower, screen, 1.0, Some(dock(1, 2))),
            Some(dock(1, 0)),
            "a moved middle panel leaves an end the pointer reaches"
        );

        // On a short work area the ends overlap; covering the pointer at both ends stays put.
        let short = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 700.0,
        };
        let overlap = (640.0, 350.0);
        assert_eq!(dock(1, 0).avoiding(overlap, short, 1.0, None), None);
        assert_eq!(dock(1, 2).avoiding(overlap, short, 1.0, None), None);
        assert_eq!(
            dock(1, 2).avoiding((640.0, 690.0), short, 1.0, None),
            Some(dock(1, 0))
        );

        let mut frame = Panel {
            rows: vec![vec![key("a", 1.0)]],
            status: String::new(),
            note: None,
            dock: Dock::default(),
            selected: None,
            status_selected: false,
            row_scan: false,
            thickness: Thickness::default(),
        }
        .frame(screen, 1.0, ScannerColor::default());
        let key_offset = frame.tiles[1].rect.y - frame.tiles[0].rect.y;
        move_frame(&mut frame, Dock::default(), dock(1, 0), screen, 1.0);
        assert_eq!(frame.tiles[0].rect, dock(1, 0).rect(screen, 1.0));
        assert_eq!(frame.tiles[1].rect.y - frame.tiles[0].rect.y, key_offset);
    }

    #[test]
    fn every_dock_anchors_the_panel_to_its_screen_position() {
        let screen = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        let rows = position_rows(|dock| dock, Dock::default());
        assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3, 3, 1]);
        let mut labels = std::collections::HashSet::new();
        for dock in rows[..3].iter().flatten().copied() {
            assert!(labels.insert(dock.label()));
            let panel = Panel {
                rows: vec![vec![key("a", 1.0)]],
                status: String::new(),
                note: None,
                dock,
                selected: None,
                status_selected: false,
                row_scan: false,
                thickness: Thickness::default(),
            };
            let rect = panel.frame(screen, 1.0, ScannerColor::default()).tiles[0].rect;
            let left = rect.x - screen.x;
            let right = screen.x + screen.width - (rect.x + rect.width);
            let above = rect.y - screen.y;
            let below = screen.y + screen.height - (rect.y + rect.height);
            let expect = |slot: u8, before: f64, after: f64| match slot {
                0 => assert_eq!(before, 0.0),
                1 => assert!((before - after).abs() < 1e-9 && before > 0.0),
                _ => assert!(after.abs() < 1e-9),
            };
            expect(dock.column, left, right);
            expect(dock.row, above, below);
        }
        assert_eq!(position_label(Dock::default(), Dock::default()), "Bottom •");
        assert_eq!(
            position_label(Dock { column: 0, row: 0 }, Dock::default()),
            "Top left"
        );
    }
}
