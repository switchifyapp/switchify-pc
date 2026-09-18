//! Switch-owned keyboard state and geometry. No native input or captured text.
use crate::{
    scan_menu::Item,
    scan_tree::{Navigator, Node, Selection},
    scanning::{Action, Frame, FrameTile, Interval, Rect, ScannerColor, MAX_SCAN_CYCLES},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Letters,
    Functions,
    Numbers,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modifier {
    #[default]
    Off,
    Once,
    Locked,
}
impl Modifier {
    fn next(self) -> Self {
        match self {
            Self::Off => Self::Once,
            Self::Once => Self::Locked,
            Self::Locked => Self::Off,
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Self::Off => "",
            Self::Once => "\nonce",
            Self::Locked => "\nlocked",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Character(char, char),
    Named(&'static str),
    Modifier(usize),
    Caps,
    Page(Page),
    Dock,
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    pub key: Key,
    pub modifiers: [bool; 4],
    pub caps: bool,
}
impl Stroke {
    pub fn character(self) -> Option<char> {
        let Key::Character(base, shifted) = self.key else {
            return None;
        };
        Some(
            if self.modifiers[0] ^ (self.caps && base.is_ascii_alphabetic()) {
                shifted
            } else {
                base
            },
        )
    }
    pub fn shortcut(self) -> bool {
        self.modifiers[1..].iter().any(|m| *m)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Stroke(Stroke),
    Close,
}

pub fn rows(page: Page, mac: bool) -> Vec<Vec<Key>> {
    use Key::{Character as C, Named as N};
    let chars = |normal: &str, shifted: &str| {
        normal
            .chars()
            .zip(shifted.chars())
            .map(|(a, b)| C(a, b))
            .collect::<Vec<_>>()
    };
    let mut result = match page {
        Page::Letters => {
            let mut number = chars("`1234567890-=", "¬!\"£$%^&*()_+");
            number.push(N("Backspace"));
            let mut top = vec![N("Tab")];
            top.extend(chars("qwertyuiop[]", "QWERTYUIOP{}"));
            let mut middle = vec![Key::Caps];
            middle.extend(chars("asdfghjkl;'#", "ASDFGHJKL:@~"));
            middle.push(N("Enter"));
            let mut bottom = vec![Key::Modifier(0)];
            bottom.extend(chars("\\zxcvbnm,./", "|ZXCVBNM<>?"));
            bottom.push(C(' ', ' '));
            vec![number, top, middle, bottom]
        }
        Page::Functions => {
            let mut result = vec![
                vec![
                    N("Escape"),
                    N("F1"),
                    N("F2"),
                    N("F3"),
                    N("F4"),
                    N("F5"),
                    N("F6"),
                ],
                vec![N("F7"), N("F8"), N("F9"), N("F10"), N("F11"), N("F12")],
                vec![N("Home"), N("ArrowUp"), N("End"), N("PageUp"), N("Delete")],
                vec![
                    N("ArrowLeft"),
                    N("ArrowDown"),
                    N("ArrowRight"),
                    N("PageDown"),
                    N("Tab"),
                    N("Enter"),
                ],
            ];
            if !mac {
                result.push(vec![
                    N("Insert"),
                    N("PrintScreen"),
                    N("ScrollLock"),
                    N("Pause"),
                ]);
            }
            result
        }
        Page::Numbers => vec![
            vec![C('7', '7'), C('8', '8'), C('9', '9'), C('/', '/')],
            vec![C('4', '4'), C('5', '5'), C('6', '6'), C('*', '*')],
            vec![C('1', '1'), C('2', '2'), C('3', '3'), C('-', '-')],
            vec![
                C('0', '0'),
                C('.', '.'),
                C('+', '+'),
                N("Enter"),
                N("Backspace"),
            ],
        ],
    };
    result.push(vec![
        Key::Modifier(0),
        Key::Modifier(1),
        Key::Modifier(2),
        Key::Modifier(3),
        Key::Caps,
    ]);
    result.push(vec![
        Key::Page(Page::Letters),
        Key::Page(Page::Functions),
        Key::Page(Page::Numbers),
        Key::Dock,
        Key::Close,
    ]);
    result
}

pub struct Keyboard {
    pub page: Page,
    pub modifiers: [Modifier; 4],
    pub caps: bool,
    pub top: bool,
    pub suspended: bool,
    pub error: bool,
    mac: bool,
    rows: Vec<Vec<Key>>,
    nav: Navigator<Key>,
    interval: Interval,
    cycles: usize,
    forward: bool,
    pending: bool,
}
impl Keyboard {
    pub fn new(mac: bool) -> Self {
        let rows = rows(Page::Letters, mac);
        let nav = Self::navigator(&rows);
        Self {
            page: Page::Letters,
            modifiers: [Modifier::Off; 4],
            caps: false,
            top: false,
            suspended: false,
            error: false,
            mac,
            rows,
            nav,
            interval: Interval::default(),
            cycles: 0,
            forward: true,
            pending: false,
        }
    }
    fn navigator(rows: &[Vec<Key>]) -> Navigator<Key> {
        Navigator::new(
            rows.iter()
                .map(|row| Node::Branch(row.iter().copied().map(Node::Leaf).collect()))
                .collect(),
        )
    }
    fn restart(&mut self) {
        self.nav.reset();
        self.interval.reset();
        self.cycles = 0;
        self.forward = true;
        self.suspended = false;
    }
    pub fn advance(&mut self, ms: u64, period: u64) {
        if !self.pending
            && !self.suspended
            && self.interval.elapsed(ms, period)
            && self.nav.step(self.forward)
        {
            self.cycles += 1;
            self.suspended = self.cycles >= MAX_SCAN_CYCLES;
        }
    }
    pub fn handle(&mut self, action: Action) -> Option<Output> {
        if self.pending {
            return None;
        }
        if self.suspended {
            if action == Action::Select {
                self.error = false;
                self.restart();
            }
            return None;
        }
        match action {
            Action::Select => {
                self.interval.reset();
                self.cycles = 0;
                match self.nav.select() {
                    Selection::Leaf(key) => return self.choose(key),
                    Selection::Entered | Selection::Escaped => self.forward = true,
                    Selection::None => {}
                }
            }
            Action::Next | Action::Back => {
                self.interval.reset();
                self.cycles = 0;
                self.forward = action == Action::Next;
                self.nav.step(self.forward);
            }
            Action::Reverse => {
                self.forward = !self.forward;
                self.interval.reset();
                self.cycles = 0;
            }
            _ => {}
        }
        None
    }
    fn choose(&mut self, key: Key) -> Option<Output> {
        match key {
            Key::Modifier(i) => self.modifiers[i] = self.modifiers[i].next(),
            Key::Caps => self.caps = !self.caps,
            Key::Dock => self.top = !self.top,
            Key::Page(page) => {
                self.page = page;
                self.rows = rows(page, self.mac);
                self.nav = Self::navigator(&self.rows);
            }
            Key::Close => return Some(Output::Close),
            Key::Character(..) | Key::Named(_) => {
                self.pending = true;
                return Some(Output::Stroke(Stroke {
                    key,
                    modifiers: self.modifiers.map(|m| m != Modifier::Off),
                    caps: self.caps,
                }));
            }
        }
        self.restart();
        None
    }
    pub fn succeeded(&mut self) {
        if self.pending {
            for modifier in &mut self.modifiers {
                if *modifier == Modifier::Once {
                    *modifier = Modifier::Off;
                }
            }
            self.pending = false;
            self.restart();
        }
    }
    pub fn failed(&mut self) {
        self.modifiers = [Modifier::Off; 4];
        self.caps = false;
        self.pending = false;
        self.restart();
        self.error = true;
        self.suspended = true;
    }
    fn label(&self, key: Key) -> String {
        match key {
            Key::Character(' ', _) => "Space".into(),
            Key::Character(..) => Stroke {
                key,
                modifiers: self.modifiers.map(|m| m != Modifier::Off),
                caps: self.caps,
            }
            .character()
            .unwrap()
            .to_string(),
            Key::Named(name) => match name {
                "ArrowUp" => "↑",
                "ArrowDown" => "↓",
                "ArrowLeft" => "←",
                "ArrowRight" => "→",
                "PageUp" => "Page Up",
                "PageDown" => "Page Down",
                "PrintScreen" => "Print Screen",
                "ScrollLock" => "Scroll Lock",
                "Escape" => "Esc",
                _ => name,
            }
            .into(),
            Key::Modifier(i) => format!(
                "{}{}",
                match i {
                    0 => "Shift",
                    1 => "Ctrl",
                    2 =>
                        if self.mac {
                            "Option"
                        } else {
                            "Alt"
                        },
                    _ =>
                        if self.mac {
                            "Command"
                        } else {
                            "Windows"
                        },
                },
                self.modifiers[i].suffix()
            ),
            Key::Caps => if self.caps { "Caps\non" } else { "Caps\noff" }.into(),
            Key::Page(page) => format!(
                "{}{}",
                match page {
                    Page::Letters => "Letters",
                    Page::Functions => "Functions",
                    Page::Numbers => "Numbers",
                },
                if page == self.page { " •" } else { "" }
            ),
            Key::Dock => if self.top {
                "Move to bottom"
            } else {
                "Move to top"
            }
            .into(),
            Key::Close => "Close".into(),
        }
    }
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let width = (1200.0 * units).min(screen.width - 16.0 * units).max(1.0);
        let height = (420.0 * units).min(screen.height / 2.0).max(1.0);
        let x = screen.x + (screen.width - width) / 2.0;
        let y = if self.top {
            screen.y
        } else {
            screen.y + screen.height - height
        };
        let row_height = height / (self.rows.len() as f64 + 0.5);
        let mut frame = Frame::default();
        let active_row = self.nav.path().first().copied().unwrap_or(self.nav.index());
        for (r, row) in self.rows.iter().enumerate() {
            let key_width = width / row.len() as f64;
            for (c, key) in row.iter().enumerate() {
                frame.tiles.push(FrameTile {
                    color,
                    text: self.label(*key),
                    icon: Item::KeyboardKey,
                    rect: Rect {
                        x: x + c as f64 * key_width + 2.0 * units,
                        y: y + r as f64 * row_height + 2.0 * units,
                        width: (key_width - 4.0 * units).max(1.0),
                        height: (row_height - 4.0 * units).max(1.0),
                    },
                    scale: units.min(row_height / 44.0).min(key_width / 65.0),
                    selected: !self.suspended
                        && r == active_row
                        && (self.nav.path().is_empty()
                            || self.nav.escaping()
                            || c == self.nav.index()),
                });
            }
        }
        let text = if self.error {
            "Input failed. Select to resume."
        } else if self.suspended {
            "Select to resume keyboard."
        } else if self.nav.escaping() {
            "Back to rows"
        } else if self.nav.path().is_empty() {
            "Choose a row"
        } else {
            "Choose a key"
        };
        frame.tiles.push(FrameTile {
            color,
            text: text.into(),
            icon: Item::KeyboardKey,
            rect: Rect {
                x,
                y: y + height - row_height * 0.5,
                width,
                height: row_height * 0.5,
            },
            scale: units.min(row_height / 44.0),
            selected: self.nav.escaping(),
        });
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uk_layout_and_platform_keys() {
        let letters = rows(Page::Letters, false).concat();
        for c in 'a'..='z' {
            assert!(letters.contains(&Key::Character(c, c.to_ascii_uppercase())));
        }
        assert!(letters.contains(&Key::Character('3', '£')));
        assert!(rows(Page::Functions, false)
            .concat()
            .contains(&Key::Named("Insert")));
        assert!(!rows(Page::Functions, true)
            .concat()
            .contains(&Key::Named("Insert")));
    }
    #[test]
    fn modifiers_are_consumed_only_after_success_and_lock_persists() {
        let mut k = Keyboard::new(false);
        k.choose(Key::Modifier(0));
        k.choose(Key::Modifier(1));
        k.choose(Key::Modifier(1));
        k.choose(Key::Page(Page::Numbers));
        k.choose(Key::Dock);
        assert_eq!(
            k.modifiers,
            [
                Modifier::Once,
                Modifier::Locked,
                Modifier::Off,
                Modifier::Off
            ]
        );
        k.choose(Key::Named("Enter"));
        assert!(k.handle(Action::Select).is_none());
        k.succeeded();
        assert_eq!(k.modifiers[0], Modifier::Off);
        assert_eq!(k.modifiers[1], Modifier::Locked);
        k.choose(Key::Modifier(1));
        assert_eq!(k.modifiers[1], Modifier::Off);
        k.choose(Key::Modifier(2));
        k.failed();
        assert_eq!(k.modifiers, [Modifier::Off; 4]);
        assert!(k.suspended);
        k.handle(Action::Select);
        assert!(!k.suspended);
    }
    #[test]
    fn caps_xor_shift_only_affects_letters() {
        let mut s = Stroke {
            key: Key::Character('a', 'A'),
            caps: true,
            modifiers: [false; 4],
        };
        assert_eq!(s.character(), Some('A'));
        s.modifiers[0] = true;
        assert_eq!(s.character(), Some('a'));
        s.key = Key::Character('3', '£');
        assert_eq!(s.character(), Some('£'));
    }
    #[test]
    fn scan_returns_to_top_and_has_row_escape_and_suspension() {
        let mut k = Keyboard::new(false);
        k.handle(Action::Select);
        assert!(!k.nav.path().is_empty());
        k.handle(Action::Back);
        assert!(k.nav.escaping());
        k.handle(Action::Select);
        assert!(k.nav.path().is_empty());
        k.handle(Action::Select);
        assert!(matches!(k.handle(Action::Select), Some(Output::Stroke(_))));
        k.succeeded();
        assert!(k.nav.path().is_empty());
        assert_eq!(k.nav.index(), 0);
        for _ in 0..k.rows.len() * MAX_SCAN_CYCLES {
            k.advance(100, 100);
        }
        assert!(k.suspended);
        assert!(k.handle(Action::Select).is_none());
        assert!(!k.suspended);
    }
    #[test]
    fn geometry_fits_work_area_at_both_docks_and_scales() {
        for units in [1.0, 1.5, 2.0] {
            for top in [false, true] {
                for page in [Page::Letters, Page::Functions, Page::Numbers] {
                    let mut k = Keyboard::new(false);
                    k.choose(Key::Page(page));
                    k.top = top;
                    let screen = Rect {
                        x: -1600.0,
                        y: -200.0,
                        width: 1600.0,
                        height: 900.0,
                    };
                    for tile in k.frame(screen, units, ScannerColor::default()).tiles {
                        assert!(tile.rect.x >= screen.x && tile.rect.y >= screen.y);
                        assert!(tile.rect.x + tile.rect.width <= screen.x + screen.width + 0.01);
                        assert!(tile.rect.y + tile.rect.height <= screen.y + screen.height + 0.01);
                    }
                }
            }
        }
    }
}
