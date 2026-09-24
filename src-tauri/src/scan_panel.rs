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

pub struct Panel {
    pub rows: Vec<Vec<PanelKey>>,
    pub status: String,
    pub top: bool,
    /// Scanner position to highlight, or `None` when no key may be chosen.
    pub selected: Option<(usize, Option<usize>)>,
    /// Highlights the status tile, used while the scan offers to go back to rows.
    pub status_selected: bool,
    pub row_scan: bool,
    pub thickness: Thickness,
}

impl Panel {
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let width = (1180.0 * units).min((screen.width - 24.0 * units).max(1.0));
        let height = (460.0 * units).min(screen.height * 0.55).max(1.0);
        let x = screen.x + (screen.width - width) / 2.0;
        let y = if self.top {
            screen.y
        } else {
            screen.y + screen.height - height
        };
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
                width,
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
            top: false,
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
            top: true,
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
}
