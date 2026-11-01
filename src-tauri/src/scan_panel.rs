//! Shared layout for switch-scanned panels: a docked background, a slim menu
//! bar and weighted rows of keys. Panels supply content; geometry lives here.
use crate::{
    panel_theme::Appearance,
    scan_preferences::{PanelSize, Thickness},
    scanning::{
        Frame, FrameLabel, FrameTile, HudPresentation, Rect, ScannerColor, TileRole, TileStyle,
    },
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
    /// Keeps its cell and scanner column but draws only a faint placeholder
    /// outline, such as an empty suggestion slot the scan skips.
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

/// Height in panel units of the menu bar along the top of every panel, as
/// in Grid 3: a slim band flush with the panel's top edge.
pub const MENU_BAR_HEIGHT: f64 = 28.0;
/// The name on the left of the menu bar.
pub const MENU_BAR_TITLE: &str = "Switchify PC";
/// The one menu bar item, on the right: it opens the Switchify PC window on Settings.
pub const SETTINGS_ITEM: &str = "Settings";
/// Width in panel units of the Settings item.
const SETTINGS_ITEM_WIDTH: f64 = 96.0;
/// Width in panel units of the title, which keeps the name at the left as in Grid 3.
const TITLE_WIDTH: f64 = 150.0;

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

/// The menu bar of a panel frame, in the frame's coordinates: the band along
/// the top of the panel background, which holds the title and the Settings item.
pub fn menu_bar(frame: &Frame) -> Option<Rect> {
    frame
        .tiles
        .iter()
        .find(|tile| tile.is_panel_background())
        .map(|background| Rect {
            height: (MENU_BAR_HEIGHT * background.scale).min(background.rect.height),
            ..background.rect
        })
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
    // A message floats beside the panel, so it moves with it.
    if let Some(label) = &mut frame.label {
        label.rect.x += to.x - from.x;
        label.rect.y += to.y - from.y;
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
    /// A failure the user must know about, such as an action that could not
    /// be done. It floats beside the panel while it applies. `None` otherwise:
    /// the highlight, or its absence, shows the state, so nothing is written
    /// out while scanning, waiting, pausing, going back or repeating.
    pub message: Option<String>,
    pub dock: Dock,
    /// Row and keys to highlight, all of the row when `None`, or `None` when no
    /// key may be chosen.
    pub selected: Option<(usize, Option<std::ops::Range<usize>>)>,
    pub row_scan: bool,
    pub thickness: Thickness,
    pub appearance: Appearance,
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
            self.appearance,
        ));
        let x = x + padding;
        let width = content_width;
        // The menu bar is a band flush with the panel's top: the name on the
        // left, which is passive, and the Settings item on the right, which
        // is clicked with the mouse. Neither is ever scanned.
        let item_width = (SETTINGS_ITEM_WIDTH * scale).min(width / 2.0);
        let item_inset = 4.0 * scale;
        frame.tiles.push(FrameTile {
            thickness: self.thickness,
            color,
            appearance: self.appearance,
            text: MENU_BAR_TITLE.into(),
            style: Some(TileStyle {
                role: TileRole::Title,
                active: false,
                row_scan: false,
            }),
            rect: Rect {
                x,
                y,
                width: (TITLE_WIDTH * scale).min((width - item_width - gap).max(1.0)),
                height: menu_bar_height,
            },
            scale,
            selected: false,
        });
        frame.tiles.push(FrameTile {
            thickness: self.thickness,
            color,
            appearance: self.appearance,
            text: SETTINGS_ITEM.into(),
            style: Some(TileStyle {
                role: TileRole::MenuBar,
                active: false,
                row_scan: false,
            }),
            rect: Rect {
                x: x + width - item_width,
                y: y + item_inset,
                width: item_width,
                height: (menu_bar_height - item_inset * 2.0).max(1.0),
            },
            scale,
            selected: false,
        });
        let y = y + menu_bar_height + padding;
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
                // A hidden key is still a slot: it draws a faint placeholder
                // outline in its cell and keeps its scanner column, but the
                // highlight never lands on it.
                frame.tiles.push(FrameTile {
                    thickness: self.thickness,
                    color,
                    appearance: self.appearance,
                    text: if key.hidden {
                        String::new()
                    } else {
                        key.text.clone()
                    },
                    style: Some(TileStyle {
                        role: if key.hidden {
                            TileRole::Placeholder
                        } else {
                            key.role
                        },
                        active: key.active && !key.hidden,
                        row_scan: self.row_scan,
                    }),
                    rect: Rect {
                        x: left,
                        y: y + r as f64 * row_height,
                        width: key_width,
                        height: (row_height - gap).max(1.0),
                    },
                    scale,
                    selected: !key.hidden
                        && self.selected.as_ref().is_some_and(|(active_row, columns)| {
                            r == *active_row
                                && columns.as_ref().is_none_or(|keys| keys.contains(&c))
                        }),
                });
                left += key_width + gap;
                c += 1;
            }
        }
        if let Some(message) = &self.message {
            frame.label = Some(message_label(message, frame.tiles[0].rect, screen, scale));
        }
        frame
    }
}

/// A message floats just above the panel, or below it when the panel is
/// docked at the top, as wide as the panel, and is drawn on the HUD so it
/// never takes a scan position.
fn message_label(text: &str, panel: Rect, screen: Rect, scale: f64) -> FrameLabel {
    let height = (MESSAGE_HEIGHT * scale).min(screen.height);
    let margin = MESSAGE_MARGIN * scale;
    let above = panel.y - height - margin;
    let y = if above >= screen.y {
        above
    } else {
        (panel.y + panel.height + margin).min(screen.y + screen.height - height)
    };
    FrameLabel {
        text: text.to_owned(),
        rect: Rect {
            x: panel.x,
            y,
            width: panel.width,
            height,
        },
        scale,
        hud: Some(HudPresentation { screen, scale }),
    }
}
/// One line of HUD text, the height the overlay hosts draw a prompt at.
pub const MESSAGE_HEIGHT: f64 = 64.0;
pub const MESSAGE_MARGIN: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;
    const FAILED: &str = "Mouse action failed · Select to resume";

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

    fn hidden_key() -> PanelKey {
        let mut hidden = key("", 1.0);
        hidden.hidden = true;
        hidden
    }

    #[test]
    fn hidden_keys_keep_their_cell_and_scanner_column() {
        let hidden = hidden_key();
        let frame = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0), hidden, key("c", 1.0)]],
            message: None,
            dock: Dock::default(),
            selected: Some((0, Some(2..3))),
            row_scan: false,
            thickness: Thickness::Thick,
            appearance: Appearance::Dark,
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
        // The background, the title, Settings, the two drawn keys and the
        // placeholder in the empty cell; nothing else.
        assert_eq!(frame.tiles.len(), 6);
        assert!(frame.label.is_none());
        assert!(c.selected && !a.selected);
        assert!(c.rect.x - a.rect.x > 2.0 * a.rect.width);
        let placeholder = frame
            .tiles
            .iter()
            .find(|t| t.style.is_some_and(|s| s.role == TileRole::Placeholder))
            .unwrap();
        assert!(placeholder.text.is_empty());
        assert!(placeholder.rect.x > a.rect.x && placeholder.rect.x < c.rect.x);
        assert_eq!(placeholder.rect.width, a.rect.width);
        // Highlighting the whole row never lights the placeholder.
        let row = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0), hidden_key(), key("c", 1.0)]],
            message: None,
            dock: Dock::default(),
            selected: Some((0, None)),
            row_scan: true,
            thickness: Thickness::Thick,
            appearance: Appearance::Dark,
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
        for tile in &row.tiles {
            let placeholder = tile.style.is_some_and(|s| s.role == TileRole::Placeholder);
            let key = tile.text == "a" || tile.text == "c";
            assert_eq!(tile.selected, key && !placeholder, "{:?}", tile.text);
        }
    }

    #[test]
    fn rows_fill_the_padded_width_with_even_gaps_below_the_menu_bar() {
        let panel = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0), key("b", 2.0)], vec![key("c", 1.0)]],
            message: None,
            dock: Dock::default(),
            selected: Some((1, None)),
            row_scan: true,
            thickness: Thickness::Thick,
            appearance: Appearance::Dark,
        };
        let screen = Rect {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default());
        let [background, title, menu_bar, a, b, c] = frame.tiles.as_slice() else {
            panic!("expected background, title, Settings and three keys");
        };
        assert!(background.is_panel_background());
        assert_eq!(background.rect.width, 1180.0);
        assert_eq!(background.rect.y + background.rect.height, 1080.0);
        let (left, right) = (background.rect.x + 16.0, background.rect.x + 1180.0 - 16.0);
        // The menu bar band is flush with the panel's top: the name on the
        // left and the Settings item on the right. Neither is scanned.
        let band = super::menu_bar(&frame).unwrap();
        assert_eq!(
            band,
            Rect {
                height: MENU_BAR_HEIGHT,
                ..background.rect
            }
        );
        assert_eq!(title.style.unwrap().role, TileRole::Title);
        assert_eq!(title.text, MENU_BAR_TITLE);
        assert_eq!((title.rect.x, title.rect.y), (left, background.rect.y));
        assert_eq!(title.rect.height, MENU_BAR_HEIGHT);
        assert!(menu_bar.is_menu_bar());
        assert_eq!(menu_bar.text, SETTINGS_ITEM);
        assert!(!title.selected && !menu_bar.selected);
        assert!((menu_bar.rect.x + menu_bar.rect.width - right).abs() < 1e-9);
        assert!(menu_bar.rect.x > title.rect.x + title.rect.width);
        assert!(menu_bar.rect.y > band.y);
        assert!(menu_bar.rect.y + menu_bar.rect.height < band.y + band.height);
        assert_eq!(a.rect.x, left);
        assert!((b.rect.x + b.rect.width - right).abs() < 1e-9);
        assert!((c.rect.x + c.rect.width - right).abs() < 1e-9);
        assert!((b.rect.width - 2.0 * a.rect.width).abs() < 1e-9);
        assert!((b.rect.x - (a.rect.x + a.rect.width) - 8.0).abs() < 1e-9);
        // The first row sits straight under the menu bar; no status line between.
        assert_eq!(a.rect.y, background.rect.y + MENU_BAR_HEIGHT + 16.0);
        assert!(menu_bar.rect.y + menu_bar.rect.height < a.rect.y);
        assert!(
            frame.label.is_none(),
            "nothing to say while scanning normally"
        );
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
            message: Some(FAILED.into()),
            dock: Dock { column: 1, row: 0 },
            selected: Some((0, None)),
            row_scan: true,
            thickness: Thickness::default(),
            appearance: Appearance::Dark,
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
        assert!(!frame.tiles[1].selected, "title");
        assert!(frame.tiles[2].is_menu_bar() && !frame.tiles[2].selected);
        assert!(frame.tiles[3].selected);
        assert_eq!(frame.tiles.len(), 4);
        // The message floats below a panel docked at the top, as wide as the
        // panel, on the HUD, and never among the tiles.
        let label = frame.label.unwrap();
        assert_eq!(label.text, FAILED);
        let panel = frame.tiles[0].rect;
        assert_eq!((label.rect.x, label.rect.width), (panel.x, panel.width));
        assert_eq!(label.rect.y, panel.y + panel.height + MESSAGE_MARGIN);
        assert_eq!(label.rect.height, MESSAGE_HEIGHT);
        assert_eq!(label.hud, Some(HudPresentation { screen, scale: 1.0 }));
    }

    #[test]
    fn a_message_floats_above_a_panel_that_has_room_and_moves_with_it() {
        let screen = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        let mut frame = Panel {
            key_width: 380.0,
            size: PanelSize::Medium,
            rows: vec![vec![key("a", 1.0)]],
            message: Some(FAILED.into()),
            dock: Dock::default(),
            selected: None,
            row_scan: false,
            thickness: Thickness::default(),
            appearance: Appearance::Dark,
        }
        .frame(screen, 1.0, ScannerColor::default());
        let panel = frame.tiles[0].rect;
        let label = frame.label.clone().unwrap();
        assert_eq!(label.text, FAILED);
        assert_eq!(label.rect.y + label.rect.height + MESSAGE_MARGIN, panel.y);
        assert_eq!((label.rect.x, label.rect.width), (panel.x, panel.width));
        move_frame(&mut frame, Dock { column: 0, row: 1 }, screen);
        let moved = frame.tiles[0].rect;
        let label = frame.label.unwrap();
        assert_eq!(label.rect.x, moved.x);
        assert_eq!(label.rect.y + label.rect.height + MESSAGE_MARGIN, moved.y);
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
            message: None,
            dock: Dock::default(),
            selected: None,
            row_scan: false,
            thickness: Thickness::default(),
            appearance: Appearance::Dark,
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
                message: None,
                dock,
                selected: None,
                row_scan: false,
                thickness: Thickness::default(),
                appearance: Appearance::Dark,
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
