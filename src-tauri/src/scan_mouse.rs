//! Switch-owned mouse panel. Native input stays in the scanning adapter.
use crate::{
    scan_items::{ItemScanner, Policy},
    scan_panel::{Dock, Panel, PanelKey},
    scan_preferences::Resolved,
    scanning::{Action, Frame, Rect, ScannerColor, TileRole},
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
    Actions,
    Keyboard,
    Dock,
    Position(Dock),
    Back,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatPrompt {
    Moving,
    Scrolling(i32),
}

pub struct MousePanel {
    pub more: bool,
    pub dock: Dock,
    pub dragging: bool,
    pub speed_percent: u16,
    pub error: bool,
    notice: Option<&'static str>,
    positioning: bool,
    displays: usize,
    rows: Vec<Vec<Key>>,
    scan: ItemScanner<Key>,
}

impl MousePanel {
    pub fn new(options: Resolved, displays: usize, speed_percent: u16) -> Self {
        let rows = Self::rows(false, false, displays);
        let scan = ItemScanner::configured_rows(&rows, Policy::KEYBOARD, options);
        Self {
            more: false,
            dock: Dock::default(),
            dragging: false,
            speed_percent,
            error: false,
            notice: None,
            positioning: false,
            displays,
            rows,
            scan,
        }
    }
    fn rows(more: bool, positioning: bool, displays: usize) -> Vec<Vec<Key>> {
        use Key::*;
        if positioning {
            crate::scan_panel::position_rows(Position, Back)
        } else if !more {
            vec![
                vec![Click, RightClick, DoubleClick, Drag],
                vec![Move(-1, -1), Move(0, -1), Move(1, -1), Scroll(1)],
                vec![Move(-1, 0), Move(1, 0)],
                vec![Move(-1, 1), Move(0, 1), Move(1, 1), Scroll(-1)],
                vec![Actions, More, Keyboard, Close],
            ]
        } else {
            let mut rows = vec![vec![Speed(-1), Speed(1)]];
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
        self.notice = None;
        match key {
            Key::More | Key::Movement => {
                self.more = key == Key::More;
                self.rebuild_rows();
            }
            Key::Dock | Key::Back => {
                self.positioning = key == Key::Dock;
                self.rebuild_rows();
            }
            Key::Position(dock) => {
                self.dock = dock;
                self.positioning = false;
                self.rebuild_rows();
            }
            // The panel is left behind.
            Key::Actions | Key::Keyboard | Key::Close => self.scan.restart(),
            _ => self.scan.continue_after_selection(),
        }
    }
    /// A repeating move or scroll was stopped by a switch.
    pub fn repeat_stopped(&mut self) {
        self.scan.continue_after_selection();
    }
    fn rebuild_rows(&mut self) {
        self.rows = Self::rows(self.more, self.positioning, self.displays);
        self.scan = ItemScanner::configured_rows(&self.rows, Policy::KEYBOARD, self.scan.options);
    }
    pub fn handle(&mut self, action: Action) -> Option<Key> {
        if self.scan.suspended && action == Action::Select {
            self.error = false;
        }
        self.scan.handle(action)
    }
    pub fn advance(&mut self, ms: u64, period: u64) {
        self.scan.advance(ms, period);
    }
    pub fn restart(&mut self) {
        self.notice = None;
        self.scan.restart();
    }
    pub fn block_actions(&mut self, message: &'static str) {
        self.notice = Some(message);
        self.scan.continue_after_selection();
    }
    pub fn suspended(&self) -> bool {
        self.scan.suspended
    }
    pub fn failed(&mut self) {
        self.scan.restart();
        self.error = true;
        self.scan.suspended = true;
    }
    pub fn set_displays(&mut self, displays: usize) {
        if self.displays != displays {
            self.displays = displays;
            if !self.positioning && self.rows != Self::rows(self.more, false, displays) {
                let (suspended, waiting) = (self.scan.suspended, self.scan.waiting());
                self.rebuild_rows();
                self.scan.suspended = suspended;
                if waiting {
                    self.scan.wait();
                }
            }
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
            Speed(-1) => format!("Slower\n{}%", self.speed_percent),
            Speed(_) => format!("Faster\n{}%", self.speed_percent),
            Monitor(-1, 0) => "Monitor left".into(),
            Monitor(0, -1) => "Monitor up".into(),
            Monitor(0, 1) => "Monitor down".into(),
            Monitor(1, 0) => "Monitor right".into(),
            Monitor(..) => "Monitor".into(),
            More => "More controls".into(),
            Movement => "Movement".into(),
            Actions => "Actions".into(),
            Keyboard => "Keyboard".into(),
            Dock => crate::scan_panel::POSITION_PAGE.into(),
            Position(dock) => crate::scan_panel::position_label(dock, self.dock),
            Back => "Back".into(),
            Close => "Switch to Point".into(),
        }
    }
    fn role(key: Key) -> TileRole {
        match key {
            Key::Move(..) => TileRole::Character,
            Key::More
            | Key::Movement
            | Key::Actions
            | Key::Keyboard
            | Key::Dock
            | Key::Back
            | Key::Close => TileRole::Toolbar,
            _ => TileRole::Utility,
        }
    }
    /// A repeating action shows its stop prompt and highlights nothing.
    pub fn frame(
        &self,
        screen: Rect,
        units: f64,
        color: ScannerColor,
        repeating: Option<RepeatPrompt>,
    ) -> Frame {
        let row_scan = self.scan.row_scan();
        let (active_row, active_column) = self.scan.position(&self.rows);
        let escaping = self.scan.nav.escaping();
        let highlight_row = row_scan || escaping;
        let status = if let Some(repeating) = repeating {
            match repeating {
                RepeatPrompt::Moving => "Moving pointer · Press and release a switch to stop",
                RepeatPrompt::Scrolling(dy) if dy > 0 => {
                    "Scrolling up · Press and release a switch to stop"
                }
                RepeatPrompt::Scrolling(_) => "Scrolling down · Press and release a switch to stop",
            }
            .to_owned()
        } else if self.error {
            "Mouse action failed · Select to resume".to_owned()
        } else if let Some(notice) = self.notice {
            notice.to_owned()
        } else if self.scan.waiting() {
            "Mouse waiting · Select to continue".to_owned()
        } else if self.scan.suspended {
            "Mouse paused · Select to resume".to_owned()
        } else if escaping {
            crate::scan_panel::BACK_TO_ROWS.to_owned()
        } else {
            crate::scan_panel::scanning_status(
                if self.positioning {
                    crate::scan_panel::POSITION_PAGE
                } else if self.more {
                    "More controls"
                } else {
                    "Movement"
                },
                row_scan,
                &self.label(self.rows[active_row][active_column.unwrap_or(0)]),
            )
        };
        let scanning = repeating.is_none() && !self.scan.suspended;
        Panel {
            rows: self
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .flat_map(|&key| {
                            let tile = PanelKey {
                                text: self.label(key),
                                weight: 1.0,
                                role: Self::role(key),
                                active: match key {
                                    Key::Drag => self.dragging,
                                    Key::Position(dock) => dock == self.dock,
                                    _ => false,
                                },
                                blank: false,
                            };
                            // Keep ← and → under the diagonal arrows.
                            let blank = matches!(key, Key::Move(_, 0)).then(PanelKey::blank);
                            std::iter::once(tile).chain(blank)
                        })
                        .collect()
                })
                .collect(),
            status,
            note: None,
            dock: self.dock,
            selected: scanning.then_some((active_row, active_column)),
            row_scan: highlight_row,
            thickness: self.scan.options.thickness,
        }
        .frame(screen, units, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_and_monitor_controls_follow_available_displays() {
        let mut panel = MousePanel::new(Resolved::default(), 1, 100);
        use Key::*;
        assert_eq!(
            panel.rows,
            [
                vec![Click, RightClick, DoubleClick, Drag],
                vec![Move(-1, -1), Move(0, -1), Move(1, -1), Scroll(1)],
                vec![Move(-1, 0), Move(1, 0)],
                vec![Move(-1, 1), Move(0, 1), Move(1, 1), Scroll(-1)],
                vec![Actions, More, Keyboard, Close],
            ]
        );
        panel.choose(Key::More);
        assert!(panel.rows.iter().flatten().any(|key| *key == Key::Dock));
        assert!(!panel.rows.iter().flatten().any(|key| *key == Key::Actions));
        assert!(!panel
            .rows
            .iter()
            .flatten()
            .any(|key| matches!(key, Key::Monitor(..) | Key::Scroll(..))));
        panel.set_displays(2);
        assert_eq!(panel.rows[1].len(), 4);
        panel.choose(Key::Movement);
        assert_eq!(panel.rows[0][0], Key::Click);
    }

    #[test]
    fn position_page_docks_the_panel_and_returns_to_the_previous_page() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let color = ScannerColor::default();
        let mut panel = MousePanel::new(Resolved::default(), 1, 100);
        panel.choose(Key::More);
        panel.choose(Key::Dock);
        assert_eq!(panel.rows[0][0], Key::Position(Dock { column: 0, row: 0 }));
        assert_eq!(panel.rows[3], [Key::Back]);
        panel.set_displays(2);
        assert_eq!(panel.rows[3], [Key::Back], "position page ignores displays");
        let frame = panel.frame(screen, 1.0, color, None);
        let current = frame.tiles.iter().find(|tile| tile.text == "Bottom •");
        assert!(current.is_some_and(|tile| tile.style.unwrap().active));
        assert_eq!(frame.tiles.last().unwrap().text, "Position · Select a row");

        panel.choose(Key::Back);
        assert!(panel.more && panel.rows[0][0] == Key::Speed(-1));
        assert_eq!(panel.dock, Dock::default());

        panel.choose(Key::Dock);
        panel.choose(Key::Position(Dock { column: 2, row: 0 }));
        assert_eq!(panel.dock, Dock { column: 2, row: 0 });
        assert_eq!(panel.rows[0][0], Key::Speed(-1));
        let background = panel.frame(screen, 1.0, color, None).tiles[0].rect;
        assert_eq!(
            (background.x + background.width, background.y),
            (1920.0, 0.0)
        );
    }

    #[test]
    fn after_an_action_the_panel_continues_as_the_user_chose() {
        use crate::scan_preferences::{NextScan, StartFrom};
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let status = |panel: &MousePanel| {
            let mut frame = panel.frame(screen, 1.0, ScannerColor::default(), None);
            frame.tiles.pop().unwrap().text
        };
        let mut panel = MousePanel::new(
            Resolved {
                next_scan: NextScan::Wait,
                start_from: StartFrom::Selection,
                ..Default::default()
            },
            1,
            100,
        );
        panel.handle(Action::Select);
        panel.handle(Action::Next);
        assert_eq!(panel.handle(Action::Select), Some(Key::RightClick));
        panel.choose(Key::RightClick);
        assert!(panel.suspended());
        assert_eq!(status(&panel), "Mouse waiting · Select to continue");
        // The monitor keys appearing does not end the wait.
        panel.set_displays(2);
        panel.set_displays(1);
        assert_eq!(status(&panel), "Mouse waiting · Select to continue");
        panel.advance(1000, 1000);
        assert_eq!(panel.handle(Action::Select), None);
        assert_eq!(status(&panel), "Movement · Select Right click");
        assert_eq!(panel.handle(Action::Select), Some(Key::RightClick));
        // A failure is not a choice, and starts again from the top.
        panel.choose(Key::RightClick);
        panel.failed();
        assert_eq!(status(&panel), "Mouse action failed · Select to resume");
        panel.handle(Action::Select);
        assert_eq!(status(&panel), "Movement · Select a row");
        // Changing page is not an action.
        panel.choose(Key::More);
        assert!(!panel.suspended());
        assert_eq!(status(&panel), "More controls · Select a row");
        // Nor is leaving the panel.
        panel.handle(Action::Next);
        panel.choose(Key::Keyboard);
        assert!(!panel.suspended());
        assert_eq!(status(&panel), "More controls · Select a row");
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
        let frame = panel.frame(screen, 1.0, ScannerColor::default(), None);
        assert!(frame.tiles.iter().any(|tile| tile.selected));
        assert!(frame
            .tiles
            .iter()
            .all(|tile| tile.rect.x >= screen.x && tile.rect.y >= screen.y));
    }

    #[test]
    fn both_mouse_pages_highlight_the_exit_row_and_return_to_it() {
        use crate::scan_preferences::{Direction, Pattern};
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        for direction in [Direction::Forward, Direction::Reverse] {
            for more in [false, true] {
                let mut panel = MousePanel::new(
                    Resolved {
                        direction,
                        ..Default::default()
                    },
                    2,
                    100,
                );
                if more {
                    panel.choose(Key::More);
                }
                let row = panel.scan.position(&panel.rows).0;
                panel.handle(Action::Select);
                panel.handle(match direction {
                    Direction::Forward => Action::Back,
                    Direction::Reverse => Action::Next,
                });
                assert!(panel.scan.nav.escaping());
                let frame = panel.frame(screen, 1.0, ScannerColor::default(), None);
                let selected: Vec<_> = frame.tiles.iter().filter(|tile| tile.selected).collect();
                assert_eq!(
                    selected.len(),
                    panel.rows[row].len(),
                    "{direction:?} more={more}"
                );
                assert!(selected
                    .iter()
                    .all(|tile| tile.rect.y == selected[0].rect.y));
                assert!(selected.iter().all(|tile| tile.style.unwrap().row_scan));
                assert!(!frame.tiles.last().unwrap().selected);
                assert_eq!(
                    frame.tiles.last().unwrap().text,
                    crate::scan_panel::BACK_TO_ROWS
                );
                panel.handle(Action::Select);
                assert_eq!(panel.scan.position(&panel.rows), (row, None));
            }
        }
        let mut linear = MousePanel::new(
            Resolved {
                pattern: Pattern::Linear,
                ..Default::default()
            },
            1,
            100,
        );
        linear.handle(Action::Back);
        assert!(!linear.scan.nav.escaping());
        let frame = linear.frame(screen, 1.0, ScannerColor::default(), None);
        assert_eq!(frame.tiles.iter().filter(|tile| tile.selected).count(), 1);
        assert!(!frame.tiles.last().unwrap().selected);
    }

    #[test]
    fn horizontal_arrows_align_with_the_diagonals_and_stay_selectable() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let mut panel = MousePanel::new(Resolved::default(), 1, 100);
        let tile = |frame: &Frame, text: &str| {
            frame
                .tiles
                .iter()
                .find(|tile| tile.text == text)
                .unwrap()
                .rect
        };
        let frame = panel.frame(screen, 1.0, ScannerColor::default(), None);
        assert_eq!(tile(&frame, "←").x, tile(&frame, "↖").x);
        assert_eq!(tile(&frame, "←").width, tile(&frame, "↖").width);
        assert_eq!(tile(&frame, "→").x, tile(&frame, "↗").x);
        assert_eq!(tile(&frame, "→").width, tile(&frame, "↗").width);
        assert_eq!(frame.tiles.len(), 1 + 18 + 1, "spacers draw no tiles");

        panel.handle(Action::Next);
        panel.handle(Action::Next);
        panel.handle(Action::Select);
        panel.handle(Action::Next);
        let frame = panel.frame(screen, 1.0, ScannerColor::default(), None);
        let selected: Vec<_> = frame.tiles.iter().filter(|tile| tile.selected).collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].text, "→");
        assert_eq!(panel.handle(Action::Select), Some(Key::Move(1, 0)));
    }

    #[test]
    fn panel_matches_keyboard_geometry_and_reports_its_state() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let color = ScannerColor::default();
        let mut panel = MousePanel::new(Resolved::default(), 1, 100);
        let frame = panel.frame(screen, 1.0, color, None);
        let keyboard = crate::scan_keyboard::Keyboard::new(false).frame(screen, 1.0, color);
        assert_eq!(frame.tiles[0].rect, keyboard.tiles[0].rect);
        assert!(frame.label.is_none());
        let status = frame.tiles.last().unwrap();
        assert_eq!(status.style.unwrap().role, TileRole::Status);
        assert_eq!(status.text, "Movement · Select a row");
        assert_eq!(status.rect.x, keyboard.tiles.last().unwrap().rect.x);
        let close = &frame.tiles[frame.tiles.len() - 2];
        assert_eq!(close.text, "Switch to Point");
        assert_eq!(close.style.unwrap().role, TileRole::Toolbar);

        panel.handle(Action::Select);
        let status = panel.frame(screen, 1.0, color, None).tiles.pop().unwrap();
        assert_eq!(status.text, "Movement · Select Left click");

        panel.choose(Key::More);
        panel.handle(Action::Select);
        let status = panel.frame(screen, 1.0, color, None).tiles.pop().unwrap();
        assert_eq!(status.text, "More controls · Select Slower 100%");

        let moving = panel.frame(screen, 1.0, color, Some(RepeatPrompt::Moving));
        assert!(moving.tiles.iter().all(|tile| !tile.selected));
        assert_eq!(
            moving.tiles.last().unwrap().text,
            "Moving pointer · Press and release a switch to stop"
        );
        for (direction, expected) in [
            (1, "Scrolling up · Press and release a switch to stop"),
            (-1, "Scrolling down · Press and release a switch to stop"),
        ] {
            let scrolling =
                panel.frame(screen, 1.0, color, Some(RepeatPrompt::Scrolling(direction)));
            assert!(scrolling.tiles.iter().all(|tile| !tile.selected));
            assert_eq!(scrolling.tiles.last().unwrap().text, expected);
        }

        panel.failed();
        let failed = panel.frame(screen, 1.0, color, None);
        assert!(failed.tiles.iter().all(|tile| !tile.selected));
        assert_eq!(
            failed.tiles.last().unwrap().text,
            "Mouse action failed · Select to resume"
        );
    }
}
