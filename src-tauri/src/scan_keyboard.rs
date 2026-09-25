//! Switch-owned keyboard state and geometry. No native input or captured text.
use crate::{
    scan_items::{ItemScanner, Policy},
    scanning::{Action, Frame, Rect, ScannerColor, TileRole, TileStyle},
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
            Self::Once => "\nNext key",
            Self::Locked => "\nLocked",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Prediction(usize),
    Character(char, char),
    Named(&'static str),
    Modifier(usize),
    Caps,
    Page(Page),
    Dock,
    Position(crate::scan_panel::Dock),
    Back,
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    pub key: Key,
    pub modifiers: [bool; 4],
    pub caps: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypingContext {
    pub foreground: usize,
    pub activity: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Punctuation {
    pub mark: char,
    pub owned_space: Option<TypingContext>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingTyped {
    Character(char),
    Punctuation(char),
    Navigation,
    Other,
    Prediction,
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
    Prediction { token: u64, index: usize },
    Stroke(Stroke),
    Punctuation(Punctuation),
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
            let mut top = vec![N("Tab")];
            top.extend(chars("qwertyuiop[]", "QWERTYUIOP{}"));
            top.push(N("Backspace"));
            let mut middle = vec![Key::Caps];
            middle.extend(chars("asdfghjkl;'#", "ASDFGHJKL:@~"));
            middle.push(N("Enter"));
            let mut bottom = vec![Key::Modifier(0)];
            bottom.extend(chars("\\zxcvbnm,./", "|ZXCVBNM<>?"));
            vec![top, middle, bottom]
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
            chars("`¬!\"£$%^&()_=", "`¬!\"£$%^&()_="),
        ],
    };
    result.push(if page == Page::Letters {
        vec![
            Key::Modifier(1),
            Key::Modifier(2),
            C(' ', ' '),
            Key::Modifier(3),
        ]
    } else {
        vec![
            Key::Modifier(0),
            Key::Modifier(1),
            Key::Modifier(2),
            C(' ', ' '),
            Key::Modifier(3),
            Key::Caps,
        ]
    });
    result.push(vec![
        Key::Close,
        Key::Page(Page::Letters),
        Key::Page(Page::Functions),
        Key::Page(Page::Numbers),
        Key::Dock,
    ]);
    result
}

pub struct Keyboard {
    pub page: Page,
    pub modifiers: [Modifier; 4],
    pub caps: bool,
    pub dock: crate::scan_panel::Dock,
    pub error: bool,
    positioning: bool,
    mac: bool,
    rows: Vec<Vec<Key>>,
    scan: ItemScanner<Key>,
    activation: Option<u64>,
    prediction_enabled: bool,
    prediction_failed: bool,
    pub(crate) prediction_loading: bool,
    predictions: Option<crate::prediction::worker::Batch>,
    queued_predictions: Option<crate::prediction::worker::Batch>,
    prefer_predictions: bool,
    wait_after_typing: bool,
    waiting_after_typing: bool,
    owned_space: Option<TypingContext>,
    capitalize_next: bool,
    auto_shift: bool,
    capital_context: Option<TypingContext>,
    pending_typed: Option<PendingTyped>,
}
impl Keyboard {
    #[cfg(test)]
    pub fn new(mac: bool) -> Self {
        Self::configured(mac, crate::scan_preferences::Resolved::default())
    }
    pub fn configured(mac: bool, options: crate::scan_preferences::Resolved) -> Self {
        let rows = rows(Page::Letters, mac);
        let scan = ItemScanner::configured_rows(&rows, Policy::KEYBOARD, options);
        Self {
            page: Page::Letters,
            modifiers: [Modifier::Off; 4],
            caps: false,
            dock: Default::default(),
            error: false,
            positioning: false,
            mac,
            rows,
            scan,
            activation: None,
            prediction_enabled: false,
            prediction_failed: false,
            prediction_loading: false,
            predictions: None,
            queued_predictions: None,
            prefer_predictions: true,
            wait_after_typing: false,
            waiting_after_typing: false,
            owned_space: None,
            capitalize_next: false,
            auto_shift: false,
            capital_context: None,
            pending_typed: None,
        }
    }
    pub fn with_wait_after_typing(mut self, enabled: bool) -> Self {
        self.wait_after_typing = enabled;
        self
    }
    pub fn enable_predictions(&mut self, enabled: bool) {
        if self.prediction_enabled == enabled {
            return;
        }
        self.prediction_enabled = enabled;
        self.rebuild_rows();
    }
    fn rebuild_rows(&mut self) {
        self.rows = if self.positioning {
            crate::scan_panel::position_rows(Key::Position, Key::Back)
        } else {
            rows(self.page, self.mac)
        };
        if self.prediction_enabled && self.page == Page::Letters && !self.positioning {
            self.rows.insert(0, (0..5).map(Key::Prediction).collect());
        }
        self.scan.replace(ItemScanner::nodes(&self.rows));
        self.scan.restart();
        self.activation = None;
        self.skip_disabled(false);
    }
    fn prediction_row_active(&self) -> bool {
        self.prediction_enabled
            && self.page == Page::Letters
            && !self.positioning
            && self.scan.position(&self.rows).0 == 0
    }
    pub fn predictions(&mut self, batch: Option<crate::prediction::worker::Batch>, failed: bool) {
        self.prediction_failed = failed;
        self.prediction_loading = false;
        if batch.is_none() {
            self.predictions = None;
            self.queued_predictions = None;
            return;
        }
        if self.waiting_after_typing {
            self.predictions = batch;
            self.queued_predictions = None;
            return;
        }
        if self.prediction_row_active() {
            if self.predictions.as_ref().map(|b| b.token) != batch.as_ref().map(|b| b.token) {
                self.predictions = None;
                self.queued_predictions = batch;
            }
        } else {
            self.predictions = batch;
            if self.prefer_predictions
                && !self.positioning
                && self.scan.options.direction == crate::scan_preferences::Direction::Forward
                && self
                    .predictions
                    .as_ref()
                    .is_some_and(|b| !b.words.is_empty())
            {
                self.scan.nav.reset();
                self.scan.reset_clock();
                self.prefer_predictions = false;
            }
        }
    }
    fn disabled(&self) -> bool {
        if !self.prediction_row_active() || self.scan.nav.escaping() {
            return false;
        }
        let count = self.predictions.as_ref().map_or(0, |b| b.words.len());
        self.scan
            .position(&self.rows)
            .1
            .map_or(count == 0, |column| column >= count)
    }
    fn skip_disabled(&mut self, automatic: bool) {
        for _ in 0..self.rows.iter().map(Vec::len).sum::<usize>() + 1 {
            if !self.disabled() {
                break;
            }
            if automatic {
                self.scan.skip_automatic();
            } else {
                self.scan.skip();
            }
        }
        if !self.prediction_row_active() {
            if let Some(batch) = self.queued_predictions.take() {
                self.predictions = Some(batch);
            }
        }
    }
    pub fn suspended(&self) -> bool {
        self.scan.suspended
    }
    fn restart(&mut self) {
        self.waiting_after_typing = false;
        self.prefer_predictions = true;
        self.scan.restart();
        self.skip_disabled(false);
    }
    pub fn advance(&mut self, ms: u64, period: u64) {
        if self.scan.advance(ms, period) {
            self.prefer_predictions = false;
            self.skip_disabled(true);
        }
    }
    #[cfg(test)]
    pub fn handle(&mut self, action: Action) -> Option<Output> {
        self.handle_with_context(action, None)
    }
    pub fn handle_with_context(
        &mut self,
        action: Action,
        context: Option<TypingContext>,
    ) -> Option<Output> {
        self.discard_stale_context(context);
        if self.scan.pending() {
            return None;
        }
        if self.scan.suspended {
            if action == Action::Select {
                self.error = false;
                self.restart();
            }
            return None;
        }
        self.prefer_predictions = false;
        if self.disabled() && action == Action::Select {
            self.scan.restart_interval();
            self.skip_disabled(false);
            return None;
        }
        if let Some(key) = self.scan.handle(action) {
            return self.choose_with_context(key, context);
        }
        self.skip_disabled(false);
        None
    }
    #[cfg(test)]
    fn choose(&mut self, key: Key) -> Option<Output> {
        self.choose_with_context(key, None)
    }
    fn stroke(&self, key: Key) -> Stroke {
        let mut modifiers = self.modifiers.map(|m| m != Modifier::Off);
        if self.auto_shift
            && (!matches!(key, Key::Character(base, _) if base.is_ascii_alphabetic())
                || modifiers[1..].iter().any(|modifier| *modifier))
        {
            modifiers[0] = false;
        }
        Stroke {
            key,
            modifiers,
            caps: self.caps,
        }
    }
    pub fn prediction_shift(&self) -> bool {
        self.modifiers[0] != Modifier::Off && !self.auto_shift
    }
    fn clear_auto_capital(&mut self) {
        if self.auto_shift {
            self.modifiers[0] = Modifier::Off;
            self.auto_shift = false;
        }
        self.capitalize_next = false;
        self.capital_context = None;
    }
    fn reconcile_auto_shift(&mut self) {
        if self.auto_shift {
            self.modifiers[0] = Modifier::Off;
            self.auto_shift = false;
        }
        if self.capitalize_next && !self.caps && self.modifiers[0] == Modifier::Off {
            self.modifiers[0] = Modifier::Once;
            self.auto_shift = true;
        }
    }
    fn smart_mark(&self, stroke: Stroke) -> Option<char> {
        if stroke.shortcut() {
            return None;
        }
        let mark = stroke.character()?;
        let supported = matches!(mark, '.' | ',' | '!' | '?' | ':' | ';');
        (supported && (self.page == Page::Letters || mark == '!')).then_some(mark)
    }
    fn choose_with_context(&mut self, key: Key, context: Option<TypingContext>) -> Option<Output> {
        self.discard_stale_context(context);
        match key {
            Key::Prediction(index) => {
                let batch = self.predictions.as_ref()?;
                if index >= batch.words.len() {
                    return None;
                }
                self.activation = self.scan.begin_activation();
                self.activation?;
                self.pending_typed = Some(PendingTyped::Prediction);
                return Some(Output::Prediction {
                    token: batch.token,
                    index,
                });
            }
            Key::Modifier(i) => {
                if i == 0 && self.capitalize_next {
                    let automatic = self.auto_shift;
                    self.clear_auto_capital();
                    if !automatic {
                        self.modifiers[i] = self.modifiers[i].next();
                    }
                } else {
                    self.modifiers[i] = self.modifiers[i].next();
                }
            }
            Key::Caps => {
                self.caps = !self.caps;
                self.reconcile_auto_shift();
            }
            Key::Page(page) => {
                self.page = page;
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
            Key::Close => return Some(Output::Close),
            Key::Character(..) | Key::Named(_) => {
                let stroke = self.stroke(key);
                if stroke.character() == Some(' ')
                    && self.owned_space.is_some()
                    && self.owned_space == context
                {
                    self.restart();
                    return None;
                }
                self.predictions = None;
                self.queued_predictions = None;
                self.activation = self.scan.begin_activation();
                self.activation?;
                if let Some(mark) = self.smart_mark(stroke) {
                    self.pending_typed = Some(PendingTyped::Punctuation(mark));
                    return Some(Output::Punctuation(Punctuation {
                        mark,
                        owned_space: self.owned_space.filter(|saved| Some(*saved) == context),
                    }));
                }
                self.pending_typed = Some(if matches!(stroke.key, Key::Named(_)) {
                    PendingTyped::Navigation
                } else {
                    stroke
                        .character()
                        .filter(|_| !stroke.shortcut())
                        .map_or(PendingTyped::Other, PendingTyped::Character)
                });
                return Some(Output::Stroke(stroke));
            }
        }
        self.restart();
        None
    }
    pub fn succeeded(&mut self) -> bool {
        self.succeeded_with_context(None)
    }
    pub fn succeeded_with_context(&mut self, context: Option<TypingContext>) -> bool {
        let completed = self
            .activation
            .take()
            .is_some_and(|revision| self.scan.complete_activation(revision));
        if completed {
            let typed = self.pending_typed.take();
            let retain_auto_shift = self.auto_shift
                && (matches!(
                    typed,
                    Some(PendingTyped::Character(character)) if !character.is_alphabetic()
                ) || matches!(
                    typed,
                    Some(PendingTyped::Punctuation(_) | PendingTyped::Prediction)
                ));
            for (index, modifier) in self.modifiers.iter_mut().enumerate() {
                if *modifier == Modifier::Once && !(index == 0 && retain_auto_shift) {
                    *modifier = Modifier::Off;
                }
            }
            match typed {
                Some(PendingTyped::Character(character)) => {
                    self.owned_space = if character == ' ' { context } else { None };
                    if character.is_alphabetic() {
                        self.clear_auto_capital();
                    }
                }
                Some(PendingTyped::Punctuation(mark)) => {
                    self.owned_space = context;
                    if matches!(mark, '.' | '!' | '?') {
                        self.capitalize_next = true;
                        self.capital_context = context;
                        self.reconcile_auto_shift();
                    }
                }
                Some(PendingTyped::Navigation) => {
                    self.owned_space = None;
                    self.clear_auto_capital();
                }
                Some(PendingTyped::Other) => {
                    self.owned_space = None;
                    self.clear_auto_capital();
                }
                Some(PendingTyped::Prediction) | None => {}
            }
            self.activation = None;
            self.restart();
            if self.wait_after_typing && self.scan.options.automatic {
                self.waiting_after_typing = true;
                self.prefer_predictions = false;
                self.scan.suspended = true;
            }
        }
        completed
    }
    pub fn prediction_inserted(
        &mut self,
        trailing_space: bool,
        contains_letter: bool,
        context: Option<TypingContext>,
    ) {
        self.owned_space = if trailing_space { context } else { None };
        if contains_letter {
            self.clear_auto_capital();
        }
    }
    fn discard_stale_context(&mut self, context: Option<TypingContext>) {
        if self.owned_space.is_some() && self.owned_space != context {
            self.owned_space = None;
        }
        if self.capital_context.is_some() && self.capital_context != context {
            self.clear_auto_capital();
        }
    }
    pub fn reset_context(&mut self) {
        self.modifiers = [Modifier::Off; 4];
        self.caps = false;
        self.activation = None;
        self.owned_space = None;
        self.capitalize_next = false;
        self.auto_shift = false;
        self.capital_context = None;
        self.pending_typed = None;
        self.predictions(None, false);
        self.restart();
    }
    pub fn failed(&mut self) {
        self.modifiers = [Modifier::Off; 4];
        self.caps = false;
        self.activation = None;
        self.owned_space = None;
        self.capitalize_next = false;
        self.auto_shift = false;
        self.capital_context = None;
        self.pending_typed = None;
        self.restart();
        self.error = true;
        self.scan.suspended = true;
    }
    fn label(&self, key: Key) -> String {
        match key {
            Key::Prediction(i) => self
                .predictions
                .as_ref()
                .and_then(|b| b.words.get(i))
                .cloned()
                .unwrap_or_default(),
            Key::Character(' ', _) => "Space".into(),
            Key::Character(..) => self.stroke(key).character().unwrap().to_string(),
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
            Key::Caps => if self.caps {
                "Caps lock\nOn"
            } else {
                "Caps lock"
            }
            .into(),
            Key::Page(page) => format!(
                "{}{}",
                match page {
                    Page::Letters => "Letters",
                    Page::Functions => "Navigation",
                    Page::Numbers => "Numbers",
                },
                if page == self.page { " •" } else { "" }
            ),
            Key::Dock => crate::scan_panel::POSITION_PAGE.into(),
            Key::Position(dock) => crate::scan_panel::position_label(dock, self.dock),
            Key::Back => "Back".into(),
            Key::Close => "Close keyboard".into(),
        }
    }
    fn weight(key: Key) -> f64 {
        match key {
            Key::Character(' ', _) => 4.0,
            Key::Named("Backspace" | "Enter") => 1.9,
            Key::Named("Tab") | Key::Caps | Key::Modifier(_) => 1.6,
            Key::Close | Key::Dock => 1.5,
            _ => 1.0,
        }
    }
    fn style(&self, key: Key, row_scan: bool) -> TileStyle {
        TileStyle {
            role: match key {
                Key::Character(..) => TileRole::Character,
                Key::Page(_) | Key::Dock | Key::Back | Key::Close => TileRole::Toolbar,
                _ => TileRole::Utility,
            },
            active: match key {
                Key::Modifier(i) => self.modifiers[i] != Modifier::Off,
                Key::Caps => self.caps,
                Key::Page(page) => self.page == page,
                Key::Position(dock) => self.dock == dock,
                _ => false,
            },
            row_scan,
        }
    }
    pub fn frame(&self, screen: Rect, units: f64, color: ScannerColor) -> Frame {
        let row_scan = self.scan.row_scan();
        let (active_row, active_column) = self.scan.position(&self.rows);
        let page = match self.page {
            _ if self.positioning => crate::scan_panel::POSITION_PAGE,
            Page::Letters => "Letters",
            Page::Functions => "Navigation",
            Page::Numbers => "Numbers",
        };
        let status = if self.error {
            "Input failed · Select to try again".to_owned()
        } else if self.waiting_after_typing {
            "Press Select to continue typing.".to_owned()
        } else if self.scan.suspended {
            "Keyboard paused · Select to resume".to_owned()
        } else if self.scan.nav.escaping() {
            crate::scan_panel::BACK_TO_ROWS.to_owned()
        } else if self.disabled() {
            "Suggestions updating · Select to continue".to_owned()
        } else if self.prediction_failed {
            "Predictions unavailable · Keyboard ready".to_owned()
        } else if self.prediction_loading {
            "Loading predictions · Keyboard ready".to_owned()
        } else {
            crate::scan_panel::scanning_status(
                page,
                row_scan,
                &self.label(self.rows[active_row][active_column.unwrap_or(0)]),
            )
        };
        crate::scan_panel::Panel {
            rows: self
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|&key| {
                            let style = self.style(key, row_scan);
                            crate::scan_panel::PanelKey {
                                text: self.label(key),
                                weight: Self::weight(key),
                                role: style.role,
                                active: style.active,
                                blank: false,
                            }
                        })
                        .collect()
                })
                .collect(),
            status,
            dock: self.dock,
            selected: (!self.scan.suspended && !self.disabled() && !self.scan.nav.escaping())
                .then_some((active_row, active_column)),
            status_selected: !self.scan.suspended && self.scan.nav.escaping(),
            row_scan,
            thickness: Default::default(),
        }
        .frame(screen, units, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(epoch: u64) -> Option<TypingContext> {
        Some(TypingContext {
            foreground: 7,
            activity: epoch,
        })
    }

    #[test]
    fn smart_punctuation_owns_spacing_and_capitalizes_one_letter() {
        let mut keyboard = Keyboard::new(false);
        let space = Key::Character(' ', ' ');
        assert!(matches!(
            keyboard.choose_with_context(space, context(1)),
            Some(Output::Stroke(_))
        ));
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.choose_with_context(space, context(1)), None);
        assert_eq!(
            keyboard.choose_with_context(Key::Character('.', '>'), context(1)),
            Some(Output::Punctuation(Punctuation {
                mark: '.',
                owned_space: context(1),
            }))
        );
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.label(Key::Character('a', 'A')), "A");
        assert_eq!(keyboard.choose_with_context(space, context(1)), None);
        assert_eq!(
            keyboard.choose_with_context(Key::Character('a', 'A'), context(1)),
            Some(Output::Stroke(Stroke {
                key: Key::Character('a', 'A'),
                modifiers: [true, false, false, false],
                caps: false,
            }))
        );
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.label(Key::Character('b', 'B')), "b");
    }

    #[test]
    fn prose_marks_use_smart_spacing_but_numeric_period_is_literal() {
        let mut keyboard = Keyboard::new(false);
        for (key, mark) in [
            (Key::Character(',', '<'), ','),
            (Key::Character(';', ':'), ';'),
            (Key::Character('.', '>'), '.'),
        ] {
            assert!(
                matches!(keyboard.choose_with_context(key, context(1)), Some(Output::Punctuation(p)) if p.mark == mark)
            );
            keyboard.succeeded_with_context(context(1));
        }
        for (key, mark) in [
            (Key::Character('/', '?'), '?'),
            (Key::Character(';', ':'), ':'),
        ] {
            keyboard.reset_context();
            keyboard.choose_with_context(Key::Modifier(0), context(1));
            assert!(
                matches!(keyboard.choose_with_context(key, context(1)), Some(Output::Punctuation(p)) if p.mark == mark)
            );
            keyboard.succeeded_with_context(context(1));
        }
        keyboard.reset_context();
        keyboard.choose_with_context(Key::Page(Page::Numbers), context(1));
        assert!(matches!(
            keyboard.choose_with_context(Key::Character('.', '.'), context(1)),
            Some(Output::Stroke(_))
        ));
        keyboard.succeeded_with_context(context(1));
        assert!(
            matches!(keyboard.choose_with_context(Key::Character('!', '!'), context(1)), Some(Output::Punctuation(p)) if p.mark == '!')
        );
    }

    #[test]
    fn external_activity_and_failed_input_discard_owned_space() {
        let mut keyboard = Keyboard::new(false);
        keyboard.choose_with_context(Key::Character(' ', ' '), context(1));
        keyboard.succeeded_with_context(context(1));
        assert!(
            matches!(keyboard.choose_with_context(Key::Character(',', '<'), context(2)), Some(Output::Punctuation(p)) if p.owned_space.is_none())
        );
        keyboard.failed();
        assert!(!keyboard.capitalize_next);
        assert_eq!(keyboard.owned_space, None);
        keyboard.handle(Action::Select);
        keyboard.choose_with_context(Key::Character('.', '>'), context(2));
        keyboard.succeeded_with_context(context(2));
        assert_eq!(keyboard.label(Key::Character('a', 'A')), "A");
        assert_eq!(keyboard.modifiers[0], Modifier::Once);
        assert!(
            matches!(keyboard.choose_with_context(Key::Character('a', 'A'), context(3)), Some(Output::Stroke(stroke)) if stroke.character() == Some('a'))
        );
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        keyboard.succeeded_with_context(context(3));
        keyboard.choose_with_context(Key::Character('.', '>'), context(3));
        keyboard.succeeded_with_context(context(3));
        keyboard.prediction_inserted(true, true, context(3));
        assert!(!keyboard.capitalize_next);
        assert_eq!(keyboard.owned_space, context(3));
        keyboard.reset_context();
        assert_eq!(keyboard.owned_space, None);
    }

    #[test]
    fn automatic_capital_does_not_toggle_manual_caps_or_change_shortcuts() {
        let mut keyboard = Keyboard::new(false);
        keyboard.choose_with_context(Key::Character('.', '>'), context(1));
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Once);
        keyboard.choose_with_context(Key::Modifier(1), context(1));
        assert!(
            matches!(keyboard.choose_with_context(Key::Character('a', 'A'), context(1)), Some(Output::Stroke(stroke)) if stroke.shortcut() && !stroke.modifiers[0])
        );
        keyboard.succeeded_with_context(context(1));
        assert!(!keyboard.capitalize_next);
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        keyboard.choose_with_context(Key::Character('.', '>'), context(1));
        keyboard.succeeded_with_context(context(1));
        assert!(keyboard.capitalize_next);
        keyboard.choose_with_context(Key::Caps, context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        assert!(
            matches!(keyboard.choose_with_context(Key::Character('a', 'A'), context(1)), Some(Output::Stroke(stroke)) if stroke.character() == Some('A') && !stroke.modifiers[0])
        );
        keyboard.succeeded_with_context(context(1));
        assert!(!keyboard.capitalize_next);
        assert!(keyboard.caps);
        assert_eq!(keyboard.label(Key::Character('b', 'B')), "B");
    }

    #[test]
    fn sentence_mark_sets_visible_one_use_shift_until_a_letter() {
        let mut keyboard = Keyboard::new(false);
        keyboard.choose_with_context(Key::Character('.', '>'), context(1));
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Once);
        assert!(!keyboard.prediction_shift());
        assert_eq!(keyboard.label(Key::Modifier(0)), "Shift\nNext key");
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let frame = keyboard.frame(screen, 1.0, ScannerColor::default());
        assert!(
            frame
                .tiles
                .iter()
                .find(|tile| tile.text.starts_with("Shift"))
                .unwrap()
                .style
                .unwrap()
                .active
        );
        assert!(matches!(
            keyboard.choose_with_context(Key::Character(',', '<'), context(1)),
            Some(Output::Punctuation(punctuation)) if punctuation.mark == ','
        ));
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Once);
        assert!(matches!(
            keyboard.choose_with_context(Key::Character('a', 'A'), context(1)),
            Some(Output::Stroke(stroke)) if stroke.modifiers[0] && stroke.character() == Some('A')
        ));
        keyboard.succeeded_with_context(context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        assert!(!keyboard.capitalize_next);
        keyboard.choose_with_context(Key::Character('.', '>'), context(1));
        keyboard.succeeded_with_context(context(1));
        keyboard.choose_with_context(Key::Modifier(0), context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        assert!(!keyboard.capitalize_next);
        keyboard.choose_with_context(Key::Modifier(0), context(1));
        assert_eq!(keyboard.modifiers[0], Modifier::Once);
        assert!(keyboard.prediction_shift());
    }

    #[test]
    fn deleting_a_sentence_boundary_clears_pending_capital() {
        let mut keyboard = Keyboard::new(false);
        keyboard.choose_with_context(Key::Character('.', '>'), context(1));
        keyboard.succeeded_with_context(context(1));
        assert!(keyboard.capitalize_next);
        keyboard.choose_with_context(Key::Named("Backspace"), context(1));
        keyboard.succeeded_with_context(context(1));
        assert!(!keyboard.capitalize_next);
        assert_eq!(keyboard.modifiers[0], Modifier::Off);
        keyboard.choose_with_context(Key::Named("Backspace"), context(1));
        keyboard.succeeded_with_context(context(1));
        assert!(matches!(
            keyboard.choose_with_context(Key::Character('a', 'A'), context(1)),
            Some(Output::Stroke(stroke)) if stroke.character() == Some('a')
        ));
    }
    #[test]
    fn typing_wait_consumes_resume_and_restarts_a_full_interval() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        for direction in [Direction::Forward, Direction::Reverse] {
            for pattern in [Pattern::Grouped, Pattern::Linear] {
                for automatic in [false, true] {
                    for enabled in [false, true] {
                        let mut k = Keyboard::configured(
                            false,
                            Resolved {
                                direction,
                                pattern,
                                automatic,
                                pass_limit: 1,
                                ..Default::default()
                            },
                        )
                        .with_wait_after_typing(enabled);
                        assert!(matches!(
                            k.choose(Key::Character('a', 'A')),
                            Some(Output::Stroke(_))
                        ));
                        assert!(k.handle(Action::Select).is_none());
                        k.succeeded();
                        assert_eq!(k.waiting_after_typing, automatic && enabled);
                        assert_eq!(k.suspended(), automatic && enabled);
                        if automatic && enabled {
                            let position = k.scan.position(&k.rows);
                            k.advance(5000, 1000);
                            assert_eq!(k.scan.position(&k.rows), position);
                            k.handle(Action::Next);
                            k.handle(Action::Back);
                            k.handle(Action::Reverse);
                            assert!(k.waiting_after_typing);
                            assert!(k.handle(Action::Select).is_none());
                            assert!(!k.waiting_after_typing);
                            k.advance(999, 1000);
                            assert_eq!(k.scan.position(&k.rows), position);
                            k.advance(1, 1000);
                            assert_ne!(k.scan.position(&k.rows), position);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn predictions_and_duplicate_completion_cannot_resume_typing_wait() {
        let mut k = Keyboard::new(false).with_wait_after_typing(true);
        k.enable_predictions(true);
        k.predictions(
            Some(crate::prediction::worker::Batch {
                token: 1,
                words: vec!["hello".into()],
            }),
            false,
        );
        assert!(matches!(
            k.choose(Key::Prediction(0)),
            Some(Output::Prediction { token: 1, index: 0 })
        ));
        k.succeeded();
        for token in 2..5 {
            k.predictions(
                Some(crate::prediction::worker::Batch {
                    token,
                    words: vec!["world".into()],
                }),
                false,
            );
            k.succeeded();
            k.advance(5000, 1000);
            assert!(k.waiting_after_typing);
            let frame = k.frame(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1000.0,
                    height: 800.0,
                },
                1.0,
                ScannerColor::Blue,
            );
            assert!(frame
                .tiles
                .iter()
                .any(|t| t.text == "Press Select to continue typing."));
            assert!(!frame.tiles.iter().any(|t| t.selected));
        }
        assert!(k.handle(Action::Select).is_none());
        assert_eq!(k.predictions.as_ref().unwrap().token, 4);
        assert!(k.handle(Action::Select).is_none());
        assert_eq!(
            k.handle(Action::Select),
            Some(Output::Prediction { token: 4, index: 0 })
        );
        assert!(k.handle(Action::Select).is_none());
    }
    #[test]
    fn keyboard_controls_remain_immediate_and_failures_keep_error_recovery() {
        for key in [
            Key::Modifier(0),
            Key::Caps,
            Key::Dock,
            Key::Page(Page::Numbers),
        ] {
            let mut k = Keyboard::new(false).with_wait_after_typing(true);
            assert!(k.choose(key).is_none());
            assert!(!k.waiting_after_typing);
            assert!(!k.suspended());
        }
        for key in [
            Key::Character(' ', ' '),
            Key::Named("Backspace"),
            Key::Named("ArrowLeft"),
        ] {
            let mut k = Keyboard::new(false).with_wait_after_typing(true);
            k.choose(key);
            k.succeeded();
            assert!(k.waiting_after_typing);
        }
        let mut k = Keyboard::new(false).with_wait_after_typing(true);
        k.choose(Key::Named("Enter"));
        k.failed();
        assert!(!k.waiting_after_typing);
        assert!(k.error);
        assert!(k.suspended());
        assert!(k.handle(Action::Select).is_none());
        assert!(!k.error);
        assert!(!k.waiting_after_typing);
    }
    #[test]
    fn arriving_predictions_receive_a_full_scan_interval() {
        let mut keyboard = Keyboard::new(false);
        keyboard.enable_predictions(true);
        keyboard.advance(490, 500);
        assert_eq!(keyboard.scan.nav.index(), 1);
        let batch = crate::prediction::worker::Batch {
            token: 1,
            words: vec!["hello".into()],
        };
        keyboard.predictions(Some(batch.clone()), false);
        assert_eq!(keyboard.scan.nav.index(), 0);
        keyboard.advance(10, 500);
        assert_eq!(keyboard.scan.nav.index(), 0);
        keyboard.predictions(Some(batch), false);
        keyboard.advance(489, 500);
        assert_eq!(keyboard.scan.nav.index(), 0);
        keyboard.advance(1, 500);
        assert_eq!(keyboard.scan.nav.index(), 1);
    }

    #[test]
    fn unchanged_predictions_preserve_navigation_timing_and_pass_limits() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        for direction in [Direction::Forward, Direction::Reverse] {
            for pattern in [Pattern::Grouped, Pattern::Linear] {
                for automatic in [false, true] {
                    let options = Resolved {
                        direction,
                        pattern,
                        pass_limit: 2,
                        ..Default::default()
                    };
                    let mut baseline = Keyboard::configured(false, options);
                    let mut polled = Keyboard::configured(false, options);
                    let batch = crate::prediction::worker::Batch {
                        token: 7,
                        words: vec!["water".into(), "walk".into()],
                    };
                    for k in [&mut baseline, &mut polled] {
                        k.enable_predictions(true);
                        k.predictions(Some(batch.clone()), false);
                        k.restart();
                    }
                    let ticks = 12 * (polled.rows.iter().map(Vec::len).sum::<usize>() + 1);
                    for tick in 0..ticks {
                        polled.predictions(Some(batch.clone()), false);
                        if automatic {
                            baseline.advance(250, 1000);
                            polled.advance(250, 1000);
                        } else if tick % 4 == 3 {
                            baseline.handle(Action::Next);
                            polled.handle(Action::Next);
                        }
                        assert_eq!(
                            polled.scan.position(&polled.rows),
                            baseline.scan.position(&baseline.rows)
                        );
                        assert_eq!(polled.suspended(), baseline.suspended());
                        assert_eq!(polled.disabled(), baseline.disabled());
                        assert_eq!(polled.predictions.as_ref().unwrap().token, 7);
                        assert!(polled.queued_predictions.is_none());
                    }
                    if automatic {
                        assert!(polled.suspended());
                    }
                }
            }
        }
    }
    #[test]
    fn unchanged_predictions_preserve_selected_key_and_execute_once() {
        let mut k = Keyboard::new(false);
        k.enable_predictions(true);
        let batch = crate::prediction::worker::Batch {
            token: 7,
            words: vec!["water".into(), "walk".into()],
        };
        k.predictions(Some(batch.clone()), false);
        k.handle(Action::Select);
        k.advance(750, 1000);
        k.predictions(Some(batch.clone()), false);
        assert_eq!(k.scan.position(&k.rows), (0, Some(0)));
        k.advance(250, 1000);
        assert_eq!(k.scan.position(&k.rows), (0, Some(1)));
        k.predictions(Some(batch), false);
        assert_eq!(
            k.handle(Action::Select),
            Some(Output::Prediction { token: 7, index: 1 })
        );
        assert!(k.handle(Action::Select).is_none());
    }
    #[test]
    fn predictions_skip_empty_slots_and_defer_acceptance() {
        let mut k = Keyboard::new(false);
        k.enable_predictions(true);
        assert_eq!(k.scan.nav.index(), 1);
        k.predictions(
            Some(crate::prediction::worker::Batch {
                token: 7,
                words: vec!["water".into(), "walk".into()],
            }),
            false,
        );
        k.restart();
        assert_eq!(k.scan.nav.index(), 0);
        assert_eq!(k.handle(Action::Select), None);
        assert_eq!(
            k.handle(Action::Select),
            Some(Output::Prediction { token: 7, index: 0 })
        );
        assert!(k.handle(Action::Select).is_none());
        k.succeeded();
        assert!(k.scan.nav.path().is_empty());
        k.handle(Action::Select);
        k.handle(Action::Next);
        k.handle(Action::Next);
        assert!(k.scan.nav.escaping());
        k.predictions(None, false);
        assert!(k.choose(Key::Prediction(0)).is_none());
        k.choose(Key::Page(Page::Numbers));
        assert!(!k
            .rows
            .iter()
            .flatten()
            .any(|k| matches!(k, Key::Prediction(_))));
    }
    #[test]
    fn rounded_panel_has_clear_corners_and_insets_all_controls() {
        let frame = Keyboard::new(false).frame(
            Rect {
                x: -1280.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            1.0,
            ScannerColor::default(),
        );
        let panel = &frame.tiles[0];
        assert!(panel.is_panel_background());
        for key in &frame.tiles[1..] {
            assert!(key.rect.x > panel.rect.x);
            assert!(key.rect.y > panel.rect.y);
            assert!(key.rect.x + key.rect.width < panel.rect.x + panel.rect.width);
            assert!(key.rect.y + key.rect.height < panel.rect.y + panel.rect.height);
        }
        let pixels = crate::scan_tile::bitmap(panel).unwrap();
        assert_eq!(pixels.data()[3], 0);
        let center = ((pixels.height() / 2 * pixels.width() + pixels.width() / 2) * 4 + 3) as usize;
        assert_eq!(pixels.data()[center], 255);
    }
    #[test]
    fn everyday_keys_are_unique_and_space_is_wide() {
        let k = Keyboard::new(false);
        let keys = k.rows.concat();
        for key in [
            Key::Caps,
            Key::Modifier(0),
            Key::Character(' ', ' '),
            Key::Close,
        ] {
            assert_eq!(
                keys.iter().filter(|candidate| **candidate == key).count(),
                1
            );
        }
        let frame = k.frame(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 680.0,
            },
            1.0,
            ScannerColor::default(),
        );
        let space = frame.tiles.iter().find(|t| t.text == "Space").unwrap();
        let letter = frame.tiles.iter().find(|t| t.text == "a").unwrap();
        assert!(space.rect.width > letter.rect.width * 3.0);
        let backspace = frame.tiles.iter().find(|t| t.text == "Backspace").unwrap();
        assert!(backspace.rect.width > letter.rect.width);
        assert_eq!(k.rows.last().unwrap()[0], Key::Close);
    }
    #[test]
    fn row_escape_only_highlights_return_and_modifier_state_is_separate() {
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 680.0,
        };
        let mut k = Keyboard::new(false);
        k.choose(Key::Modifier(0));
        let active = k.frame(screen, 1.0, ScannerColor::default());
        let shift = active
            .tiles
            .iter()
            .find(|t| t.text.starts_with("Shift"))
            .unwrap();
        assert!(shift.style.unwrap().active);
        assert!(!shift.selected);
        k.handle(Action::Select);
        let keys = k.frame(screen, 1.0, ScannerColor::default());
        assert_eq!(keys.tiles.iter().filter(|t| t.selected).count(), 1);
        k.handle(Action::Back);
        let escaping = k.frame(screen, 1.0, ScannerColor::default());
        let selected: Vec<_> = escaping.tiles.iter().filter(|t| t.selected).collect();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].text.starts_with("Back to rows"));
        k.handle(Action::Select);
        assert!(k.scan.nav.path().is_empty());
    }
    #[test]
    fn uk_layout_and_platform_keys() {
        let letters = rows(Page::Letters, false).concat();
        for c in 'a'..='z' {
            assert!(letters.contains(&Key::Character(c, c.to_ascii_uppercase())));
        }
        assert!(!letters
            .iter()
            .any(|key| matches!(key, Key::Character(c, _) if c.is_ascii_digit())));
        assert!(rows(Page::Letters, false)[0].contains(&Key::Named("Backspace")));
        let numbers = rows(Page::Numbers, false).concat();
        for symbol in "`1234567890-=¬!\"£$%^&*()_+".chars() {
            assert!(
                numbers
                    .iter()
                    .any(|key| matches!(key, Key::Character(a, b) if *a == symbol || *b == symbol)),
                "Missing {symbol}"
            );
        }
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
        k.choose(Key::Position(crate::scan_panel::Dock { column: 0, row: 1 }));
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
        assert!(k.suspended());
        k.handle(Action::Select);
        assert!(!k.suspended());
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
        assert!(!k.scan.nav.path().is_empty());
        k.handle(Action::Back);
        assert!(k.scan.nav.escaping());
        k.handle(Action::Select);
        assert!(k.scan.nav.path().is_empty());
        k.handle(Action::Select);
        assert!(matches!(k.handle(Action::Select), Some(Output::Stroke(_))));
        k.succeeded();
        assert!(k.scan.nav.path().is_empty());
        assert_eq!(k.scan.nav.index(), 0);
        for _ in 0..k.rows.len() * k.scan.options.pass_limit {
            k.advance(100, 100);
        }
        assert!(k.suspended());
        assert!(k.handle(Action::Select).is_none());
        assert!(!k.suspended());
    }
    #[test]
    fn geometry_fits_work_area_at_every_dock_and_scale() {
        let docks = crate::scan_panel::position_rows(|dock| dock, Default::default());
        for units in [0.75, 1.0, 1.5, 2.0, 3.0] {
            for &dock in docks.iter().flatten() {
                for page in [Page::Letters, Page::Functions, Page::Numbers] {
                    let mut k = Keyboard::new(false);
                    k.choose(Key::Page(page));
                    k.dock = dock;
                    let screen = Rect {
                        x: -1600.0,
                        y: -200.0,
                        width: 800.0,
                        height: 600.0,
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
    #[test]
    fn position_page_replaces_predictions_and_returns_to_the_page() {
        use crate::scan_panel::Dock;
        let mut k = Keyboard::new(false);
        k.enable_predictions(true);
        k.choose(Key::Page(Page::Numbers));
        k.choose(Key::Page(Page::Letters));
        k.choose(Key::Dock);
        assert_eq!(k.rows.len(), 4);
        assert_eq!(k.rows[1][1], Key::Position(Dock { column: 1, row: 1 }));
        assert!(!k.prediction_row_active());
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let frame = k.frame(screen, 1.0, ScannerColor::default());
        assert_eq!(frame.tiles.last().unwrap().text, "Position · Select a row");
        assert!(frame.tiles.iter().any(|tile| tile.text == "Bottom •"));

        k.choose(Key::Back);
        assert_eq!(k.rows[0][0], Key::Prediction(0));
        assert_eq!(k.dock, Dock::default());

        k.choose(Key::Dock);
        k.choose(Key::Position(Dock { column: 0, row: 1 }));
        assert_eq!(k.rows[0][0], Key::Prediction(0));
        let background = k.frame(screen, 1.0, ScannerColor::default()).tiles[0].rect;
        assert_eq!(background.x, 0.0);
        assert!(background.y > 0.0 && background.y + background.height < 1080.0);
    }
    #[test]
    fn reverse_scanning_counts_passes_across_empty_prediction_slots() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        for pattern in [Pattern::Grouped, Pattern::Linear] {
            let mut k = Keyboard::configured(
                false,
                Resolved {
                    direction: Direction::Reverse,
                    pattern,
                    pass_limit: 1,
                    ..Default::default()
                },
            );
            k.enable_predictions(true);
            let steps = if pattern == Pattern::Grouped {
                k.rows.len() - 1
            } else {
                k.rows.iter().skip(1).map(Vec::len).sum()
            };
            for _ in 0..steps - 1 {
                k.advance(100, 100);
                assert!(!k.suspended());
            }
            k.advance(100, 100);
            assert!(k.suspended());
            assert!(k.handle(Action::Select).is_none());
            assert!(!k.suspended());
            for _ in 0..steps * 2 {
                k.handle(Action::Back);
            }
            assert!(!k.suspended());
        }
    }
    #[test]
    fn prediction_warning_does_not_hide_resume_or_error_instructions() {
        let mut k = Keyboard::new(false);
        k.prediction_failed = true;
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };
        k.scan.suspended = true;
        let frame = k.frame(screen, 1.0, ScannerColor::Blue);
        assert!(frame
            .tiles
            .iter()
            .any(|tile| tile.text.contains("Select to resume")));
        k.failed();
        let frame = k.frame(screen, 1.0, ScannerColor::Blue);
        assert!(frame
            .tiles
            .iter()
            .any(|tile| tile.text.contains("Select to try again")));
    }
    #[test]
    fn arriving_predictions_preserve_reverse_origin_and_full_pass() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        for pattern in [Pattern::Grouped, Pattern::Linear] {
            let mut k = Keyboard::configured(
                false,
                Resolved {
                    direction: Direction::Reverse,
                    pattern,
                    pass_limit: 1,
                    ..Default::default()
                },
            );
            k.enable_predictions(true);
            let origin = k.scan.nav.index();
            k.advance(490, 500);
            k.predictions(
                Some(crate::prediction::worker::Batch {
                    token: 1,
                    words: vec!["hello".into(), "world".into()],
                }),
                false,
            );
            assert_eq!(k.scan.nav.index(), origin);
            k.advance(10, 500);
            assert_ne!(k.scan.nav.index(), origin);
            assert!(!k.suspended());
            let count = if pattern == Pattern::Grouped {
                k.rows.len()
            } else {
                k.rows.iter().skip(1).map(Vec::len).sum::<usize>() + 2
            };
            for _ in 1..count - 1 {
                k.advance(500, 500);
                assert!(!k.suspended());
            }
            k.advance(500, 500);
            assert!(k.suspended());
        }
    }
    #[test]
    fn invalid_predictions_wait_for_permitted_movement_and_count_reverse_wraps() {
        use crate::scan_preferences::{Direction, Pattern, Resolved};
        for pattern in [Pattern::Grouped, Pattern::Linear] {
            for replacement in [false, true] {
                for manual in [false, true] {
                    let mut k = Keyboard::configured(
                        false,
                        Resolved {
                            direction: Direction::Reverse,
                            pattern,
                            pass_limit: 1,
                            automatic: !manual,
                            ..Default::default()
                        },
                    );
                    k.enable_predictions(true);
                    k.predictions(
                        Some(crate::prediction::worker::Batch {
                            token: 1,
                            words: vec!["hello".into(), "world".into()],
                        }),
                        false,
                    );
                    let count = if pattern == Pattern::Grouped {
                        k.rows.len()
                    } else {
                        k.rows.iter().skip(1).map(Vec::len).sum::<usize>() + 2
                    };
                    for _ in 0..count - 1 {
                        k.advance(500, 500);
                    }
                    assert!(k.prediction_row_active());
                    k.advance(490, 500);
                    let position = k.scan.nav.index();
                    let next = replacement.then(|| crate::prediction::worker::Batch {
                        token: 2,
                        words: vec!["new".into()],
                    });
                    for _ in 0..3 {
                        k.predictions(next.clone(), false);
                    }
                    assert_eq!(k.scan.nav.index(), position);
                    assert!(!k.suspended());
                    let frame = k.frame(
                        Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 1000.0,
                            height: 800.0,
                        },
                        1.0,
                        ScannerColor::Blue,
                    );
                    assert!(!frame.tiles.iter().any(|tile| tile.selected));
                    if manual {
                        assert!(k.handle(Action::Select).is_none());
                        assert!(!k.suspended());
                        assert!(!k.prediction_row_active());
                    } else {
                        k.advance(9, 500);
                        assert_eq!(k.scan.nav.index(), position);
                        assert!(!k.suspended());
                        k.advance(1, 500);
                        assert!(k.suspended());
                    }
                }
            }
        }
    }
}
