//! Switch-owned mouse panel. Native input stays in the scanning adapter.
use crate::{
    scan_items::{ItemScanner, Policy},
    scan_menu::Item,
    scan_preferences::Resolved,
    scanning::{Action, Frame, FrameLabel, FrameTile, Rect, ScannerColor, TileRole, TileStyle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Move(i8, i8),
    Click,
    RightClick,
    DoubleClick,
    Drag,
    Scroll(i8),
    Speed(i8),
    Monitor(i8, i8),
    More,
    Movement,
    Keyboard,
    Dock,
    Close,
}

pub struct MousePanel {
    pub more: bool,
    pub top: bool,
    pub dragging: bool,
    pub speed_percent: u8,
    pub error: bool,
    displays: usize,
    rows: Vec<Vec<Key>>,
    scan: ItemScanner<Key>,
}

impl MousePanel {
    pub fn new(options: Resolved, displays: usize, speed_percent: u8) -> Self {
        let rows = Self::rows(false, displays);
        let scan = ItemScanner::configured_rows(&rows, Policy::KEYBOARD, options);
        Self {
            more: false,
            top: false,
            dragging: false,
            speed_percent,
            error: false,
            displays,
            rows,
            scan,
        }
    }
    fn rows(more: bool, displays: usize) -> Vec<Vec<Key>> {
        use Key::*;
        if !more {
            vec![
                vec![Move(-1, -1), Move(0, -1), Move(1, -1)],
                vec![Move(-1, 0), Click, Move(1, 0)],
                vec![Move(-1, 1), Move(0, 1), Move(1, 1)],
                vec![RightClick, DoubleClick, Drag],
                vec![More, Keyboard, Dock, Close],
            ]
        } else {
            let mut rows = vec![vec![Scroll(1), Scroll(-1)], vec![Speed(-1), Speed(1)]];
            if displays > 1 {
                rows.push(vec![
                    Monitor(-1, 0),
                    Monitor(0, -1),
                    Monitor(0, 1),
                    Monitor(1, 0),
                ]);
            }
            rows.push(vec![Movement, Keyboard, Dock, Close]);
            rows
        }
    }
    pub fn choose(&mut self, key: Key) {
        match key {
            Key::More | Key::Movement => {
                self.more = key == Key::More;
                self.rows = Self::rows(self.more, self.displays);
                self.scan =
                    ItemScanner::configured_rows(&self.rows, Policy::KEYBOARD, self.scan.options);
            }
            Key::Dock => {
                self.top = !self.top;
                self.scan.restart();
            }
            _ => self.scan.restart(),
        }
    }
    pub fn handle(&mut self, action: Action) -> Option<Key> {
        if self.scan.suspended {
            if action == Action::Select {
                self.error = false;
                self.scan.restart();
            }
            return None;
        }
        self.scan.handle(action)
    }
    pub fn advance(&mut self, ms: u64, period: u64) {
        self.scan.advance(ms, period);
    }
    pub fn restart(&mut self) {
        self.scan.restart();
    }
    pub fn suspended(&self) -> bool {
        self.scan.suspended
    }
    pub fn failed(&mut self) {
        self.error = true;
        self.scan.suspended = true;
    }
    pub fn set_displays(&mut self, displays: usize) {
        if self.displays != displays {
            self.displays = displays;
            self.rows = Self::rows(self.more, displays);
            self.scan =
                ItemScanner::configured_rows(&self.rows, Policy::KEYBOARD, self.scan.options);
        }
    }
    fn label(&self, key: Key) -> String {
        use Key::*;
        match key {
            Move(-1, -1) => "↖".into(),
            Move(0, -1) => "↑".into(),
            Move(1, -1) => "↗".into(),
            Move(-1, 0) => "←".into(),
            Move(1, 0) => "→".into(),
            Move(-1, 1) => "↙".into(),
            Move(0, 1) => "↓".into(),
            Move(1, 1) => "↘".into(),
            Move(..) => "Move".into(),
            Click => "Left click".into(),
            RightClick => "Right click".into(),
            DoubleClick => "Double click".into(),
            Drag => if self.dragging {
                "End drag"
            } else {
                "Start drag"
            }
            .into(),
            Scroll(1) => "Scroll up".into(),
            Scroll(_) => "Scroll down".into(),
            Speed(-1) => format!("Slower · {}%", self.speed_percent),
            Speed(_) => format!("Faster · {}%", self.speed_percent),
            Monitor(-1, 0) => "Monitor left".into(),
            Monitor(0, -1) => "Monitor up".into(),
            Monitor(0, 1) => "Monitor down".into(),
            Monitor(1, 0) => "Monitor right".into(),
            Monitor(..) => "Monitor".into(),
            More => "More controls".into(),
            Movement => "Movement".into(),
            Keyboard => "Keyboard".into(),
            Dock => if self.top { "Dock bottom" } else { "Dock top" }.into(),
            Close => "Switch to Point".into(),
        }
    }
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(1) as f64;
        let width = (columns * 160.0 + 32.0)
            .min(screen.width / units.max(0.1) - 8.0)
            .max(1.0)
            * units;
        let height = (self.rows.len() as f64 * 98.0 + 52.0)
            .min(screen.height / units.max(0.1) * 0.65)
            .max(1.0)
            * units;
        let x = screen.x + (screen.width - width) / 2.0;
        let y = if self.top {
            screen.y
        } else {
            screen.y + screen.height - height
        };
        let scale = units
            .min(width / (columns * 160.0 + 32.0))
            .min(height / (self.rows.len() as f64 * 98.0 + 52.0));
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
        let (active_row, active_column) = self.scan.position(&self.rows);
        let row_height = (height - 52.0 * scale) / self.rows.len() as f64;
        for (r, row) in self.rows.iter().enumerate() {
            let cell_width = (width - 24.0 * scale) / row.len() as f64;
            for (c, key) in row.iter().enumerate() {
                frame.tiles.push(FrameTile {
                    thickness: self.scan.options.thickness,
                    color,
                    text: self.label(*key),
                    rect: Rect {
                        x: x + 12.0 * scale + c as f64 * cell_width,
                        y: y + 44.0 * scale + r as f64 * row_height,
                        width: (cell_width - 6.0 * scale).max(1.0),
                        height: (row_height - 6.0 * scale).max(1.0),
                    },
                    scale,
                    icon: Item::KeyboardKey,
                    style: Some(TileStyle {
                        role: TileRole::Utility,
                        active: *key == Key::Drag && self.dragging,
                        row_scan: self.scan.row_scan(),
                    }),
                    selected: !self.scan.suspended
                        && !self.scan.nav.escaping()
                        && active_row == r
                        && active_column.is_none_or(|column| column == c),
                });
            }
        }
        frame.label = Some(FrameLabel {
            text: if self.error {
                "Mouse action failed · Select to resume".into()
            } else if self.more {
                "Mouse · More controls".into()
            } else {
                "Mouse · Movement".into()
            },
            rect: Rect {
                x: x + 12.0 * scale,
                y: y + 4.0 * scale,
                width: width - 24.0 * scale,
                height: 38.0 * scale,
            },
            scale: scale * 0.8,
            hud: None,
        });
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_and_monitor_controls_follow_available_displays() {
        let mut panel = MousePanel::new(Resolved::default(), 1, 100);
        assert_eq!(panel.rows[1][1], Key::Click);
        assert_eq!(panel.rows[0].len(), 3);
        panel.choose(Key::More);
        assert!(!panel
            .rows
            .iter()
            .flatten()
            .any(|key| matches!(key, Key::Monitor(..))));
        panel.set_displays(2);
        assert_eq!(panel.rows[2].len(), 4);
        panel.choose(Key::Movement);
        assert_eq!(panel.rows[1][1], Key::Click);
    }

    #[test]
    fn panel_renders_with_selected_tiles_inside_the_screen() {
        let panel = MousePanel::new(Resolved::default(), 2, 100);
        let screen = Rect {
            x: -1280.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default());
        assert!(frame.tiles.iter().any(|tile| tile.selected));
        assert!(frame
            .tiles
            .iter()
            .all(|tile| tile.rect.x >= screen.x && tile.rect.y >= screen.y));
    }
}
