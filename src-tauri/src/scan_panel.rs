//! Shared layout for switch-scanned panels: a docked background, a status tile
//! and weighted rows of keys. Panels supply content; geometry lives here.
use crate::{
    scan_items::ItemScanner,
    scan_preferences::{PanelSize, Thickness},
    scanning::{Frame, FrameTile, Rect, ScannerColor, TileRole, TileStyle},
};

/// One scale for the complete panel, including its text.
pub fn fit_scale(screen: Rect, units: f64, size: PanelSize, width: f64, height: f64) -> f64 {
    (units * size.scale())
        .min(screen.width.max(1.0) / width)
        .min(screen.height.max(1.0) / height)
}

pub struct PanelKey {
    pub text: String,
    pub weight: f64,
    pub role: TileRole,
    pub active: bool,
    /// Reserves a cell without drawing it or counting it as a scanner column.
    pub blank: bool,
    /// Keeps its cell and scanner column without drawing it, such as an
    /// empty suggestion slot the scan skips.
    pub hidden: bool,
}

impl PanelKey {
    pub fn blank() -> Self {
        Self {
            text: String::new(),
            weight: 1.0,
            role: TileRole::Utility,
            active: false,
            blank: true,
            hidden: false,
        }
    }
}

/// One of nine screen positions. `column` and `row` are 0 (start), 1 (middle) or 2 (end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub fn rect(self, screen: Rect, (width, height): (f64, f64)) -> Rect {
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
    ///
    /// `menu_bar` is the panel's menu bar, in screen coordinates, when the panel is at this
    /// dock. The pointer over a menu bar never moves its panel, wherever the panel is drawn, so
    /// the bar can be clicked.
    pub fn avoiding(
        self,
        pointer: (f64, f64),
        screen: Rect,
        size: (f64, f64),
        moved: Option<Dock>,
        menu_bar: Option<Rect>,
    ) -> Option<Dock> {
        let (x, y) = pointer;
        let within = |rect: Rect| {
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        };
        let covers = |dock: Dock| within(dock.rect(screen, size));
        let home = self.rect(screen, size);
        let menu_bar_at = |dock: Dock| {
            let rect = dock.rect(screen, size);
            menu_bar.map(|bar| {
                rendered_bounds(Rect {
                    x: bar.x - home.x + rect.x,
                    y: bar.y - home.y + rect.y,
                    ..bar
                })
            })
        };
        if let Some(moved) = moved.filter(|&moved| menu_bar_at(moved).is_some_and(within)) {
            return Some(moved);
        }
        if !covers(self) {
            return None;
        }
        // A panel at home stays under a pointer on its bar. One already moved
        // away does not come back under a pointer crossing its home bar.
        if moved.is_none() && menu_bar_at(self).is_some_and(within) {
            return None;
        }
        let rect = self.rect(screen, size);
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

/// Height in panel units of the menu bar along the top of every panel, including its gap.
pub const MENU_BAR_HEIGHT: f64 = 36.0;
/// The one menu bar item: it opens the Switchify PC window on Settings.
pub const SETTINGS_ITEM: &str = "Settings";

/// The whole pixels a tile covers once drawn. Windows places a tile window at
/// its rounded origin with a rounded size; macOS snaps each edge. This covers
/// both, so a pointer on any drawn pixel of a menu bar counts as over it.
pub fn rendered_bounds(rect: Rect) -> Rect {
    let left = rect.x.min(rect.x.round());
    let top = rect.y.min(rect.y.round());
    let right = (rect.x + rect.width)
        .max(rect.x.round() + rect.width.round())
        .max((rect.x + rect.width).round());
    let bottom = (rect.y + rect.height)
        .max(rect.y.round() + rect.height.round())
        .max((rect.y + rect.height).round());
    Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

/// The menu bar of a panel frame, in the frame's coordinates.
pub fn menu_bar(frame: &Frame) -> Option<Rect> {
    frame
        .tiles
        .iter()
        .find(|tile| tile.is_menu_bar())
        .map(|tile| tile.rect)
}

/// Shifts a panel frame drawn at `from` so it is drawn at `to` instead.
pub fn move_frame(frame: &mut Frame, to: Dock, screen: Rect) {
    let Some(background) = frame.tiles.iter().find(|tile| tile.is_panel_background()) else {
        return;
    };
    let from = background.rect;
    let to = to.rect(screen, (from.width, from.height));
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
    pub key_width: f64,
    pub size: PanelSize,
    pub rows: Vec<Vec<PanelKey>>,
    pub status: String,
    /// Passive feedback beside the scanning prompt; never a scan target.
    pub note: Option<String>,
    pub dock: Dock,
    /// Row and keys to highlight, all of the row when `None`, or `None` when no
    /// key may be chosen.
    pub selected: Option<(usize, Option<std::ops::Range<usize>>)>,
    pub row_scan: bool,
    /// Passive text drawn across a row, such as an empty suggestion row whose
    /// keys are all hidden; never a scan target.
    pub row_label: Option<(usize, String)>,
    pub thickness: Thickness,
}

impl Panel {
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let preferred_width = self
            .rows
            .iter()
            .map(|row| {
                row.iter().map(|key| key.weight).sum::<f64>() * self.key_width
                    + row.len().saturating_sub(1) as f64 * 8.0
            })
            .fold(0.0, f64::max)
            + 32.0;
        let preferred_height = 32.0
            + MENU_BAR_HEIGHT
            + 48.0
            + self.rows.len() as f64 * 56.0
            + self.rows.len().saturating_sub(1) as f64 * 8.0;
        let scale = fit_scale(screen, units, self.size, preferred_width, preferred_height);
        let Rect {
            x,
            y,
            width,
            height,
        } = self
            .dock
            .rect(screen, (preferred_width * scale, preferred_height * scale));
        let padding = 16.0 * scale;
        let content_width = width - padding * 2.0;
        let gap = 8.0 * scale;
        let row_height = 64.0 * scale;
        let header_height = 48.0 * scale;
        let menu_bar_height = MENU_BAR_HEIGHT * scale;
        let mut frame = Frame::default();
        frame.tiles.push(FrameTile::panel_background(
            Rect {
                x,
                y,
                width,
                height,
            },
            scale,
            color,
        ));
        let x = x + padding;
        let y = y + padding;
        let width = content_width;
        // The menu bar is the first thing the pointer meets from above. It is
        // clicked with the mouse, never scanned, so it has no selection.
        frame.tiles.push(FrameTile {
            thickness: self.thickness,
            color,
            text: SETTINGS_ITEM.into(),
            style: Some(TileStyle {
                role: TileRole::MenuBar,
                active: false,
                row_scan: false,
            }),
            rect: Rect {
                x,
                y,
                width,
                height: (menu_bar_height - gap).max(1.0),
            },
            scale,
            selected: false,
        });
        let y = y + menu_bar_height;
        for (r, row) in self.rows.iter().enumerate() {
            let total_weight: f64 = row.iter().map(|key| key.weight).sum();
            let cell_unit =
                (width - gap * row.len().saturating_sub(1) as f64).max(1.0) / total_weight;
            let mut left = x;
            let mut c = 0;
            for key in row {
                let key_width = cell_unit * key.weight;
                if key.blank {
                    left += key_width + gap;
                    continue;
                }
                if key.hidden {
                    left += key_width + gap;
                    c += 1;
                    continue;
                }
                frame.tiles.push(FrameTile {
                    thickness: self.thickness,
                    color,
                    text: key.text.clone(),
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
                    selected: self.selected.as_ref().is_some_and(|(active_row, columns)| {
                        r == *active_row && columns.as_ref().is_none_or(|keys| keys.contains(&c))
                    }),
                });
                left += key_width + gap;
                c += 1;
            }
        }
        if let Some((row, text)) = &self.row_label {
            frame.tiles.push(FrameTile {
                thickness: self.thickness,
                color,
                text: text.clone(),
                style: Some(TileStyle {
                    role: TileRole::Status,
                    active: false,
                    row_scan: false,
                }),
                rect: Rect {
                    x,
                    y: y + header_height + *row as f64 * row_height,
                    width,
                    height: (row_height - gap).max(1.0),
                },
                scale,
                selected: false,
            });
        }
        let note_width = self.note.as_ref().map_or(0.0, |_| width * 0.28);
        if let Some(note) = &self.note {
            frame.tiles.push(FrameTile {
                thickness: self.thickness,
                color,
                text: note.clone(),
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
            selected: false,
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
/// `scanning_status` for an item scanner, which may be choosing a group.
pub fn item_status<T: Clone>(page: &str, scan: &ItemScanner<T>, key: &str) -> String {
    if scan.group_scan() {
        format!("{page} · Select a group")
    } else {
        scanning_status(page, scan.row_scan(), key)
    }
}
/// The status while an item scanner offers to go back.
pub fn back_status<T: Clone>(scan: &ItemScanner<T>) -> &'static str {
    if scan.escaping_to_groups() {
        BACK_TO_GROUPS
    } else {
        BACK_TO_ROWS
    }
}
pub const BACK_TO_ROWS: &str = "Back to rows · Select to return";
pub const BACK_TO_GROUPS: &str = "Back to groups · Select to return";

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
            hidden: false,
        }
    }

    #[test]
    fn hidden_keys_keep_their_cell_and_scanner_column() {
        let mut hidden = key("", 1.0);
        hidden.hidden = true;
        let frame = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0), hidden, key("c", 1.0)]],
            status: "Page · Select a key".into(),
            note: None,
            dock: Dock::default(),
            selected: Some((0, Some(2..3))),
            row_scan: false,
            row_label: None,
            thickness: Thickness::Thick,
        }
        .frame(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            1.0,
            ScannerColor::default(),
        );
        let a = frame.tiles.iter().find(|t| t.text == "a").unwrap();
        let c = frame.tiles.iter().find(|t| t.text == "c").unwrap();
        // The background, the menu bar, the two drawn keys and the status line.
        assert_eq!(frame.tiles.len(), 5);
        assert!(c.selected && !a.selected);
        assert!(c.rect.x - a.rect.x > 2.0 * a.rect.width);
    }

    #[test]
    fn rows_fill_the_padded_width_with_even_gaps_below_the_status_tile() {
        let panel = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0), key("b", 2.0)], vec![key("c", 1.0)]],
            status: "Page · Select a row".into(),
            note: None,
            dock: Dock::default(),
            selected: Some((1, None)),
            row_scan: true,
            row_label: None,
            thickness: Thickness::Thick,
        };
        let screen = Rect {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default());
        let [background, menu_bar, a, b, c, status] = frame.tiles.as_slice() else {
            panic!("expected background, menu bar, three keys and a status tile");
        };
        assert!(background.is_panel_background());
        assert_eq!(background.rect.width, 1180.0);
        assert_eq!(background.rect.y + background.rect.height, 1080.0);
        let (left, right) = (background.rect.x + 16.0, background.rect.x + 1180.0 - 16.0);
        // The menu bar spans the padded width above the status tile, and is never scanned.
        assert!(menu_bar.is_menu_bar());
        assert_eq!(menu_bar.text, SETTINGS_ITEM);
        assert!(!menu_bar.selected);
        assert_eq!(
            (menu_bar.rect.x, menu_bar.rect.y, menu_bar.rect.width),
            (left, background.rect.y + 16.0, right - left)
        );
        assert_eq!(menu_bar.rect.height, MENU_BAR_HEIGHT - 8.0);
        assert_eq!(super::menu_bar(&frame), Some(menu_bar.rect));
        assert_eq!(a.rect.x, left);
        assert!((b.rect.x + b.rect.width - right).abs() < 1e-9);
        assert!((c.rect.x + c.rect.width - right).abs() < 1e-9);
        assert!((b.rect.width - 2.0 * a.rect.width).abs() < 1e-9);
        assert!((b.rect.x - (a.rect.x + a.rect.width) - 8.0).abs() < 1e-9);
        assert_eq!(status.style.unwrap().role, TileRole::Status);
        assert_eq!(
            (status.rect.x, status.rect.y),
            (left, background.rect.y + 16.0 + MENU_BAR_HEIGHT)
        );
        assert!(menu_bar.rect.y + menu_bar.rect.height < status.rect.y);
        assert!(status.rect.y + status.rect.height < a.rect.y);
        assert_eq!(
            frame.tiles.iter().filter(|tile| tile.selected).count(),
            1,
            "row scan highlights only row 1"
        );
        assert!(c.selected && c.thickness == Thickness::Thick);
    }

    #[test]
    fn docking_and_row_escape_selection() {
        let panel = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0)]],
            status: BACK_TO_ROWS.into(),
            note: None,
            dock: Dock { column: 1, row: 0 },
            selected: Some((0, None)),
            row_scan: true,
            row_label: None,
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
        assert!(frame.tiles[1].is_menu_bar() && !frame.tiles[1].selected);
        assert!(frame.tiles[2].selected);
        assert!(!frame.tiles[3].selected);
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
            let bottom = dock(column, 2).rect(screen, (1180.0, 460.0));
            let inside = (bottom.x + 1.0, bottom.y + bottom.height - 1.0);
            let outside = (bottom.x + 1.0, bottom.y - 1.0);
            assert_eq!(
                dock(column, 2).avoiding(inside, screen, (1180.0, 460.0), None, None),
                Some(dock(column, 0))
            );
            assert_eq!(
                dock(column, 2).avoiding(outside, screen, (1180.0, 460.0), None, None),
                None
            );
            let top = dock(column, 0).rect(screen, (1180.0, 460.0));
            assert_eq!(
                dock(column, 0).avoiding((top.x, top.y), screen, (1180.0, 460.0), None, None),
                Some(dock(column, 2))
            );
        }
        let middle = dock(1, 1).rect(screen, (1180.0, 460.0));
        let upper = (middle.x + 10.0, middle.y + 10.0);
        let lower = (middle.x + 10.0, middle.y + middle.height - 10.0);
        assert_eq!(
            dock(1, 1).avoiding(upper, screen, (1180.0, 460.0), None, None),
            Some(dock(1, 2))
        );
        assert_eq!(
            dock(1, 1).avoiding(lower, screen, (1180.0, 460.0), None, None),
            Some(dock(1, 0))
        );
        let centre = (middle.x + 10.0, middle.y + middle.height / 2.0 + 1.0);
        assert_eq!(
            dock(1, 1).avoiding(centre, screen, (1180.0, 460.0), Some(dock(1, 2)), None),
            Some(dock(1, 2)),
            "a moved middle panel stays put while its end is clear of the pointer"
        );
        assert_eq!(
            dock(1, 1).avoiding(lower, screen, (1180.0, 460.0), Some(dock(1, 2)), None),
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
        assert_eq!(
            dock(1, 0).avoiding(overlap, short, (1180.0, 460.0), None, None),
            None
        );
        assert_eq!(
            dock(1, 2).avoiding(overlap, short, (1180.0, 460.0), None, None),
            None
        );
        assert_eq!(
            dock(1, 2).avoiding((640.0, 690.0), short, (1180.0, 460.0), None, None),
            Some(dock(1, 0))
        );

        // The menu bar is clicked, so the pointer over it moves nothing: not a
        // panel at home, and not one already moved out of the way.
        let bottom = dock(1, 2).rect(screen, (1180.0, 460.0));
        let bar = Rect {
            x: bottom.x + 16.0,
            y: bottom.y + 16.0,
            width: 1180.0 - 32.0,
            height: 28.0,
        };
        let on_bar = (bar.x + 1.0, bar.y + 1.0);
        let below_bar = (bar.x + 1.0, bar.y + bar.height + 1.0);
        assert_eq!(
            dock(1, 2).avoiding(on_bar, screen, (1180.0, 460.0), None, Some(bar)),
            None
        );
        assert_eq!(
            dock(1, 2).avoiding(below_bar, screen, (1180.0, 460.0), None, Some(bar)),
            Some(dock(1, 0))
        );
        let top = dock(1, 0).rect(screen, (1180.0, 460.0));
        let moved_bar = (bar.x + 1.0, bar.y - bottom.y + top.y + 1.0);
        assert_eq!(
            dock(1, 2).avoiding(
                moved_bar,
                screen,
                (1180.0, 460.0),
                Some(dock(1, 0)),
                Some(bar)
            ),
            Some(dock(1, 0)),
            "a moved panel stays while the pointer is on its bar"
        );
        assert_eq!(
            dock(1, 2).avoiding(moved_bar, screen, (1180.0, 460.0), None, Some(bar)),
            None,
            "but an unmoved panel is not moved under the pointer"
        );
        assert_eq!(
            dock(1, 2).avoiding(on_bar, screen, (1180.0, 460.0), Some(dock(1, 0)), Some(bar)),
            Some(dock(1, 0)),
            "a moved panel does not come back under a pointer crossing its home bar"
        );

        // The drawn bar is the rounded window; its edge pixels count as the bar.
        let fractional = Rect {
            x: bottom.x + 16.4,
            y: bottom.y + 15.6,
            width: 1148.2,
            height: 27.6,
        };
        let drawn = rendered_bounds(fractional);
        assert!(drawn.x <= fractional.x && drawn.x <= fractional.x.round());
        assert!(drawn.y <= fractional.y && drawn.y <= fractional.y.round());
        assert!(drawn.x + drawn.width >= fractional.x.round() + fractional.width.round());
        assert!(drawn.y + drawn.height >= (fractional.y + fractional.height).round());
        let left_edge = (fractional.x.round(), fractional.y.round() + 10.0);
        assert_eq!(
            dock(1, 2).avoiding(left_edge, screen, (1180.0, 460.0), None, Some(fractional)),
            None,
            "the drawn left edge of the bar is the bar"
        );

        let mut frame = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0)]],
            status: String::new(),
            note: None,
            dock: Dock::default(),
            selected: None,
            row_scan: false,
            row_label: None,
            thickness: Thickness::default(),
        }
        .frame(screen, 1.0, ScannerColor::default());
        let key_offset = frame.tiles[1].rect.y - frame.tiles[0].rect.y;
        let size = (frame.tiles[0].rect.width, frame.tiles[0].rect.height);
        move_frame(&mut frame, dock(1, 0), screen);
        assert_eq!(frame.tiles[0].rect, dock(1, 0).rect(screen, size));
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
                key_width: 380.0,
                size: PanelSize::Medium,
                rows: vec![vec![key("a", 1.0)]],
                status: String::new(),
                note: None,
                dock,
                selected: None,
                row_scan: false,
                row_label: None,
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
