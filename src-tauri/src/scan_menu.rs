//! Menu rows and navigation are independent of native windows and input.
use crate::{
    scan_tree::{Navigator, Node, Selection},
    scanning::{Action, Frame, FrameLabel, FrameTile, Interval, Rect, MAX_SCAN_CYCLES},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
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
    Back,
    DragHere,
    DestinationAgain,
    CancelDrag,
}
impl Item {
    pub fn label(self) -> &'static str {
        match self {
            Self::LeftClick => "Left click",
            Self::RightClick => "Right click",
            Self::DoubleClick => "Double click",
            Self::Scroll => "Scroll",
            Self::Drag => "Drag",
            Self::NewPoint => "New point",
            Self::Cancel => "Cancel",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Back => "Back to actions",
            Self::DragHere => "Drag here",
            Self::DestinationAgain => "New destination",
            Self::CancelDrag => "Cancel drag",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Actions,
    Scroll,
    ConfirmDrag,
}
pub struct Menu {
    pub kind: Kind,
    rows: Vec<Vec<Item>>,
    nav: Navigator<Item>,
    interval: Interval,
    period: u64,
    cycles: usize,
    forward: bool,
    pub suspended: bool,
}
impl Menu {
    pub fn new(kind: Kind, period: u64) -> Self {
        use Item::*;
        let rows = match kind {
            Kind::Actions => vec![
                vec![LeftClick, RightClick, DoubleClick],
                vec![Scroll, Drag],
                vec![NewPoint, Cancel],
            ],
            Kind::Scroll => vec![vec![Up, Down], vec![Left, Right], vec![Back]],
            Kind::ConfirmDrag => vec![vec![DragHere, DestinationAgain, CancelDrag]],
        };
        let nav = Navigator::new(
            rows.iter()
                .map(|row| Node::Branch(row.iter().copied().map(Node::Leaf).collect()))
                .collect(),
        );
        Self {
            kind,
            rows,
            nav,
            interval: Interval::default(),
            period,
            cycles: 0,
            forward: true,
            suspended: false,
        }
    }
    pub fn restart_interval(&mut self) {
        self.interval.reset();
        self.cycles = 0;
        self.suspended = false;
    }
    pub fn advance(&mut self, ms: u64) {
        if !self.suspended && self.interval.elapsed(ms, self.period) && self.nav.step(self.forward)
        {
            self.cycles += 1;
            self.suspended = self.cycles >= MAX_SCAN_CYCLES;
        }
    }
    pub fn handle(&mut self, action: Action) -> Option<Item> {
        if self.suspended {
            if action == Action::Select {
                self.restart_interval();
            }
            return None;
        }
        match action {
            Action::Select => {
                self.restart_interval();
                match self.nav.select() {
                    Selection::Leaf(item) => return Some(item),
                    Selection::Entered | Selection::Escaped => self.forward = true,
                    Selection::None => {}
                }
            }
            Action::Next | Action::Back => {
                self.interval.reset();
                self.forward = action == Action::Next;
                self.nav.step(self.forward);
            }
            Action::Reverse => {
                self.forward = !self.forward;
                self.restart_interval();
            }
            _ => {}
        }
        None
    }
    pub fn frame(&self, point: (i32, i32), screen: Rect, units: f64) -> Frame {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(1) as f64;
        let logical_width = 16.0 + columns * 180.0;
        let logical_height = 56.0 + 180.0 * self.rows.len() as f64;
        let scale = units
            .min(screen.width / logical_width)
            .min(screen.height / logical_height);
        let width = logical_width * scale;
        let height = logical_height * scale;
        let panel = place(point, screen, width, height, 20.0 * scale);
        let mut frame = Frame::default();
        let active_row = self.nav.path().first().copied().unwrap_or(self.nav.index());
        for (r, row) in self.rows.iter().enumerate() {
            let tile_width = 180.0 * scale;
            for (c, item) in row.iter().enumerate() {
                let rect = Rect {
                    x: panel.x + 8.0 * scale + c as f64 * tile_width,
                    y: panel.y + (56.0 + 180.0 * r as f64) * scale,
                    width: 168.0 * scale,
                    height: 168.0 * scale,
                };
                let selected = r == active_row
                    && (self.nav.path().is_empty() || self.nav.escaping() || c == self.nav.index());
                frame.tiles.push(FrameTile {
                    text: item.label().into(),
                    rect,
                    scale,
                    icon: *item,
                    selected,
                });
            }
        }
        let text = if self.suspended {
            "Select to resume"
        } else if self.nav.escaping() {
            "Back to rows"
        } else {
            match self.kind {
                Kind::Actions => "Choose an action",
                Kind::Scroll => "Scroll at selected point",
                Kind::ConfirmDrag => "Confirm drag",
            }
        };
        frame.label = Some(FrameLabel {
            text: text.into(),
            rect: Rect {
                height: 48.0 * scale,
                ..panel
            },
            scale: scale * 0.75,
        });
        frame
    }
}
pub fn outline(r: Rect, t: f64) -> [Rect; 4] {
    [
        Rect { height: t, ..r },
        Rect {
            y: r.y + r.height - t,
            height: t,
            ..r
        },
        Rect { width: t, ..r },
        Rect {
            x: r.x + r.width - t,
            width: t,
            ..r
        },
    ]
}
fn place(point: (i32, i32), screen: Rect, width: f64, height: f64, gap: f64) -> Rect {
    let (x, y) = (point.0 as f64, point.1 as f64);
    let candidates = [
        (x + gap, y - height / 2.0),
        (x - gap - width, y - height / 2.0),
        (x - width / 2.0, y + gap),
        (x - width / 2.0, y - gap - height),
    ];
    let (x, y) = candidates
        .into_iter()
        .find(|(x, y)| {
            *x >= screen.x
                && *y >= screen.y
                && *x + width <= screen.x + screen.width
                && *y + height <= screen.y + screen.height
        })
        .unwrap_or(candidates[0]);
    Rect {
        x: x.clamp(screen.x, screen.x + screen.width - width),
        y: y.clamp(screen.y, screen.y + screen.height - height),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_keeps_square_columns_and_highlights_the_current_row_or_item() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 240.0,
        };
        let mut menu = Menu::new(Kind::Actions, 250);
        let row = menu.frame((10, 10), screen, 2.0);
        assert_eq!(row.tiles.iter().filter(|t| t.selected).count(), 3);
        assert_eq!(row.tiles[0].rect.x, row.tiles[3].rect.x);
        assert_eq!(row.tiles[1].rect.x, row.tiles[4].rect.x);
        for tile in &row.tiles {
            assert_eq!(tile.rect.width, tile.rect.height);
        }
        menu.handle(Action::Select);
        let item = menu.frame((10, 10), screen, 2.0);
        assert_eq!(item.tiles.iter().filter(|t| t.selected).count(), 1);
        assert!(item.tiles[0].selected);
    }
    #[test]
    fn layouts_fit_edges_negative_coordinates_and_scaling() {
        let screen = Rect {
            x: -1920.0,
            y: -200.0,
            width: 1920.0,
            height: 1080.0,
        };
        for point in [
            (-1920, -200),
            (-1, -200),
            (-1920, 879),
            (-1, 879),
            (-900, 300),
        ] {
            for scale in [1.0, 1.5, 2.0] {
                let frame = Menu::new(Kind::Actions, 250).frame(point, screen, scale);
                for rect in frame
                    .tiles
                    .iter()
                    .map(|tile| tile.rect)
                    .chain(frame.label.iter().map(|label| label.rect))
                {
                    assert!(rect.x >= screen.x && rect.y >= screen.y);
                    assert!(rect.x + rect.width <= screen.x + screen.width + 0.001);
                    assert!(rect.y + rect.height <= screen.y + screen.height + 0.001);
                }
            }
        }
    }
    #[test]
    fn escape_manual_timing_and_reverse_use_shared_navigator() {
        let mut m = Menu::new(Kind::Actions, 250);
        m.handle(Action::Select);
        m.advance(249);
        m.handle(Action::Back);
        m.advance(1);
        assert!(m.nav.escaping());
        assert_eq!(m.handle(Action::Select), None);
        assert!(m.nav.path().is_empty());
        assert_eq!(m.nav.index(), 0);
        m.handle(Action::Reverse);
        m.advance(250);
        assert_eq!(m.nav.index(), 2);
    }
}
