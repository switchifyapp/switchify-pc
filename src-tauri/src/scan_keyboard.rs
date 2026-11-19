//! Switch-owned keyboard state and geometry. No native input or captured text.
use crate::{
    custom_panels::Cell,
    panel_actions::PanelAction,
    panel_extras::PanelExtras,
    scan_items::{ItemScanner, Policy},
    scanning::{Action, Frame, Rect, ScannerColor, TileRole, TileStyle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Letters,
    Functions,
    Numbers,
    Symbols,
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
    RetryPrediction,
    Character(char, char),
    Named(&'static str),
    Modifier(usize),
    Caps,
    Page(Page),
    Layout,
    Dock,
    Position(crate::scan_panel::Dock),
    Back,
    Close,
    /// A tile the user added below the keys: its row and place in the row.
    Extra(usize, usize),
    /// A tile the user added beside the keys: its column and place in it.
    Column(usize, usize),
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
/// Where Close keyboard returns to, shown on the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseTo {
    Home,
    MousePanel,
    PointScanning,
}
impl CloseTo {
    /// Named like the Home tiles, and kept to two short lines: the key is
    /// narrow on most pages, and a taller label would be cut off.
    fn label(self) -> &'static str {
        match self {
            Self::Home => "Close to\nHome",
            Self::MousePanel => "Close to\nMouse",
            Self::PointScanning => "Close to\nPoint",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Prediction {
        token: u64,
        index: usize,
    },
    RetryPrediction,
    Stroke(Stroke),
    Punctuation(Punctuation),
    Layout(crate::point_scan::KeyboardLayout),
    Close,
    /// A tile the user added was chosen; the workflow runs its action.
    Tile(Key),
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
            top.extend(chars("qwertyuiop", "QWERTYUIOP"));
            top.push(N("Backspace"));
            let mut middle = vec![Key::Caps];
            // Punctuation keys type what they show, with or without Shift.
            middle.extend(chars("asdfghjkl'", "ASDFGHJKL'"));
            middle.push(N("Enter"));
            let mut bottom = vec![Key::Modifier(0)];
            bottom.extend(chars("zxcvbnm,.?", "ZXCVBNM,.?"));
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
        ],
        Page::Symbols => SYMBOL_ROWS
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let mut keys = chars(row, row);
                if index == SYMBOL_ROWS.len() - 1 {
                    keys.extend([N("Enter"), N("Backspace")]);
                }
                keys
            })
            .collect(),
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
    result.push(page_row());
    result
}

/// Every symbol has its own key, which types what it shows. Common
/// punctuation comes first, then brackets, maths, and the rarer symbols.
const SYMBOL_ROWS: [&str; 4] = ["@.,?!'\"-_/:;", "()[]{}", "+=*%^<>", "£$&#~\\|`¬"];

fn page_row() -> Vec<Key> {
    vec![
        Key::Close,
        Key::Page(Page::Letters),
        Key::Page(Page::Functions),
        Key::Page(Page::Numbers),
        Key::Page(Page::Symbols),
        Key::Dock,
    ]
}

fn rows_for(page: Page, mac: bool, layout: crate::point_scan::KeyboardLayout) -> Vec<Vec<Key>> {
    use crate::point_scan::KeyboardLayout;
    use Key::{Character as C, Named as N};
    if page != Page::Letters || layout == KeyboardLayout::Qwerty {
        let mut result = rows(page, mac);
        result.last_mut().unwrap().push(Key::Layout);
        return result;
    }
    let letters = |text: &str| {
        text.chars()
            .map(|letter| C(letter, letter.to_ascii_uppercase()))
            .collect::<Vec<_>>()
    };
    if layout == KeyboardLayout::SimpleQwerty {
        let mut top = letters("qwertyuiop");
        top.push(N("Backspace"));
        let mut middle = letters("asdfghjkl");
        middle.push(N("Enter"));
        let mut bottom = vec![Key::Modifier(0)];
        bottom.extend(letters("zxcvbnm"));
        // A full stop that stays one with Shift or Caps on.
        bottom.push(C('.', '.'));
        let modifiers = vec![
            Key::Modifier(1),
            Key::Modifier(2),
            C(' ', ' '),
            Key::Modifier(3),
        ];
        let mut pages = page_row();
        pages.push(Key::Layout);
        return vec![top, middle, bottom, modifiers, pages];
    }
    let mut first = vec![C(' ', ' ')];
    first.extend(letters("etaoi"));
    first.push(N("Backspace"));
    let mut second = letters("nshrdlu");
    second.push(N("Enter"));
    let mut fourth = letters("gkjqxz");
    fourth.extend([N("Tab"), Key::Caps]);
    let mut pages = page_row();
    pages.push(Key::Layout);
    vec![
        first,
        second,
        letters("cmfwypvb"),
        fourth,
        vec![C(',', ','), C('.', '.'), C('\'', '\''), C('?', '?')],
        vec![
            Key::Modifier(0),
            Key::Modifier(1),
            Key::Modifier(2),
            Key::Modifier(3),
        ],
        pages,
    ]
}

pub struct Keyboard {
    pub page: Page,
    pub layout: crate::point_scan::KeyboardLayout,
    pub modifiers: [Modifier; 4],
    pub caps: bool,
    pub dock: crate::scan_panel::Dock,
    pub error: bool,
    layout_save_error: bool,
    positioning: bool,
    mac: bool,
    /// The user's tiles, shown on every page: rows above the page row,
    /// columns down the right.
    extras: PanelExtras,
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
    /// Where Close keyboard goes, once the workflow has said.
    close_to: Option<CloseTo>,
}
impl Keyboard {
    pub fn set_close_to(&mut self, close_to: CloseTo) {
        self.close_to = Some(close_to);
    }
    #[cfg(all(test, target_os = "windows"))]
    pub(crate) fn fixture_page(&mut self, key: Key) {
        self.choose(key);
    }

    pub fn set_panel_size(&mut self, size: crate::scan_preferences::PanelSize) {
        self.scan.options.panel_size = size;
    }
    pub fn set_panel_theme(&mut self, theme: crate::scan_preferences::PanelTheme) {
        self.scan.options.panel_theme = theme;
    }

    #[cfg(test)]
    pub fn new(mac: bool) -> Self {
        Self::configured(mac, crate::scan_preferences::Resolved::default())
    }
    pub fn configured(mac: bool, options: crate::scan_preferences::Resolved) -> Self {
        let rows = rows_for(
            Page::Letters,
            mac,
            crate::point_scan::KeyboardLayout::Qwerty,
        );
        let scan = ItemScanner::configured_rows(&rows, Policy::KEYBOARD, options);
        Self {
            page: Page::Letters,
            layout: crate::point_scan::KeyboardLayout::Qwerty,
            modifiers: [Modifier::Off; 4],
            caps: false,
            dock: Default::default(),
            error: false,
            layout_save_error: false,
            positioning: false,
            mac,
            extras: PanelExtras::default(),
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
            close_to: None,
        }
    }
    pub fn with_wait_after_typing(mut self, enabled: bool) -> Self {
        self.wait_after_typing = enabled;
        self
    }
    pub fn with_layout(mut self, layout: crate::point_scan::KeyboardLayout) -> Self {
        self.set_layout(layout);
        self
    }
    /// Adds the user's tiles around the keys.
    pub fn with_extras(mut self, extras: PanelExtras) -> Self {
        self.extras = extras;
        self.rebuild_rows();
        self
    }
    /// Replaces the user's tiles while the keyboard is open. The page and a
    /// pause stay; scanning starts again from the top.
    pub fn set_extras(&mut self, extras: PanelExtras) {
        self.extras = extras;
        self.rebuild_rows_keeping_pause();
    }
    /// The built-in keys of the QWERTY Letters page, for a picture of the
    /// keyboard.
    pub fn fixed_picture(mac: bool) -> Vec<Vec<(String, f64)>> {
        Self::layout_picture(mac, crate::point_scan::KeyboardLayout::Qwerty)
    }
    pub(crate) fn layout_picture(
        mac: bool,
        layout: crate::point_scan::KeyboardLayout,
    ) -> Vec<Vec<(String, f64)>> {
        let keyboard = Self::configured(mac, Default::default()).with_layout(layout);
        keyboard
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&key| (keyboard.display(key), keyboard.weight(key)))
                    .collect()
            })
            .collect()
    }
    /// The tile a key the user added stands for.
    pub fn cell(&self, key: Key) -> Option<&Cell> {
        match key {
            Key::Extra(row, column) => self.extras.rows.get(row)?.get(column),
            Key::Column(column, index) => self.extras.columns.get(column)?.get(index),
            _ => None,
        }
    }
    /// The widest row of keys, in key widths, with the gaps between them.
    /// The user's tiles are left out: they take their width from the keys.
    /// The suggestion row fits itself to this width, so it is not counted.
    fn widest_keys(&self) -> f64 {
        let gap = crate::scan_panel::KEY_GAP;
        self.rows
            .iter()
            .filter(|keys| !keys.iter().any(|key| matches!(key, Key::Prediction(_))))
            .map(|keys| {
                let fixed: Vec<Key> = keys
                    .iter()
                    .copied()
                    .filter(|key| !matches!(key, Key::Extra(..) | Key::Column(..)))
                    .collect();
                if fixed.is_empty() {
                    return 0.0;
                }
                fixed.iter().map(|&key| self.weight(key)).sum::<f64>()
                    + fixed.len().saturating_sub(1) as f64 * gap / 68.0
            })
            .fold(1.0, f64::max)
    }
    /// A column is as wide as its widest tile.
    fn column_width(&self, column: usize) -> f64 {
        self.extras.columns.get(column).map_or(1.0, |cells| {
            cells
                .iter()
                .map(|cell| f64::from(cell.span))
                .fold(1.0, f64::max)
        })
    }
    /// The columns drawn on the current page: none while positioning.
    fn drawn_columns(&self) -> usize {
        if self.positioning {
            0
        } else {
            self.extras.columns.len()
        }
    }
    /// The blank that keeps a row's keys to the widest row's width once
    /// columns follow them, so every column lines up. A row a column added
    /// below the keyboard is blank across the whole width of the keys.
    fn row_filler(&self, row: &[Key]) -> Option<crate::scan_panel::PanelKey> {
        if self.drawn_columns() == 0 {
            return None;
        }
        let gap = crate::scan_panel::KEY_GAP;
        let keys: Vec<Key> = row
            .iter()
            .copied()
            .filter(|key| !matches!(key, Key::Column(..)))
            .collect();
        let widest = self.widest_keys() * 68.0;
        let slack = if keys.is_empty() {
            widest
        } else {
            let width = keys.iter().map(|&key| self.weight(key)).sum::<f64>() * 68.0
                + keys.len().saturating_sub(1) as f64 * gap;
            widest - width - gap
        };
        (slack > 0.5).then(|| {
            let mut blank = crate::scan_panel::PanelKey::blank();
            blank.weight = slack / 68.0;
            blank
        })
    }
    /// A tile's share of its row. An extra row shares the width of the
    /// keyboard's widest row of keys between its tiles by span, as a row on
    /// the user's panels does, so the keys decide how wide the keyboard is.
    fn extra_weight(&self, row: usize, column: usize) -> f64 {
        let Some(cells) = self.extras.rows.get(row) else {
            return 1.0;
        };
        let gap = crate::scan_panel::KEY_GAP;
        let keys = self.widest_keys() - cells.len().saturating_sub(1) as f64 * gap / 68.0;
        let total: f64 = cells.iter().map(|cell| f64::from(cell.span)).sum();
        let span = cells.get(column).map_or(1.0, |cell| f64::from(cell.span));
        span * keys.max(1.0) / total.max(1.0)
    }
    /// A tile the user added could not act. The scan carries on.
    pub fn cancel_activation(&mut self) {
        self.activation = None;
        self.pending_typed = None;
        self.restart();
    }
    pub fn set_layout(&mut self, layout: crate::point_scan::KeyboardLayout) {
        if self.layout != layout {
            self.layout = layout;
            self.rebuild_rows_keeping_pause();
        }
        self.layout_save_error = false;
    }
    pub fn layout_change_failed(&mut self) {
        self.layout_save_error = true;
        self.restart();
    }
    pub fn enable_predictions(&mut self, enabled: bool) {
        if self.prediction_enabled == enabled {
            return;
        }
        self.prediction_enabled = enabled;
        self.prediction_loading = enabled;
        self.prediction_failed = false;
        self.rebuild_rows_keeping_pause();
    }
    /// Rebuilds the rows without ending a pause or a wait for Select.
    fn rebuild_rows_keeping_pause(&mut self) {
        let (suspended, waiting) = (self.scan.suspended, self.scan.waiting());
        self.rebuild_rows();
        self.scan.suspended = suspended;
        if waiting {
            self.scan.wait();
        }
    }
    fn rebuild_rows(&mut self) {
        self.rows = if self.positioning {
            crate::scan_panel::position_rows(Key::Position, Key::Back)
        } else {
            rows_for(self.page, self.mac, self.layout)
        };
        if !self.positioning && !self.extras.rows.is_empty() {
            let bottom = self.rows.pop().unwrap_or_default();
            self.rows
                .extend(self.extras.rows.iter().enumerate().map(|(row, cells)| {
                    (0..cells.len())
                        .map(|column| Key::Extra(row, column))
                        .collect()
                }));
            self.rows.push(bottom);
        }
        if self.prediction_enabled && self.page == Page::Letters && !self.positioning {
            self.rows.insert(
                0,
                (0..crate::prediction::SLOTS).map(Key::Prediction).collect(),
            );
        }
        if self.prediction_enabled && self.prediction_failed && !self.positioning {
            self.rows.last_mut().unwrap().push(Key::RetryPrediction);
        }
        if !self.positioning {
            // Columns run down the right from the top row; a column longer
            // than the keyboard adds rows of its own below.
            let lengths: Vec<usize> = self.extras.columns.iter().map(Vec::len).collect();
            for (column, length) in lengths.into_iter().enumerate() {
                for index in 0..length {
                    if index >= self.rows.len() {
                        self.rows.push(Vec::new());
                    }
                    self.rows[index].push(Key::Column(column, index));
                }
            }
        }
        self.scan.replace_rows(&self.rows);
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
    pub(crate) fn displayed_prediction(&self) -> Option<u64> {
        self.predictions.as_ref().map(|b| b.token)
    }
    pub fn predictions(&mut self, batch: Option<crate::prediction::worker::Batch>, failed: bool) {
        let retry_changed = self.prediction_failed != failed;
        self.prediction_failed = failed;
        self.prediction_loading = false;
        if failed {
            self.predictions = None;
            self.queued_predictions = None;
        }
        if retry_changed {
            self.rebuild_rows_keeping_pause();
        }
        if batch.is_none() {
            self.predictions = None;
            self.queued_predictions = None;
            if retry_changed && !self.scan.suspended {
                self.skip_disabled(false);
            }
            return;
        }
        if self.waiting_after_typing {
            self.predictions = batch;
            self.queued_predictions = None;
            return;
        }
        if self.prediction_row_active() {
            if self.predictions.as_ref().map(|b| b.token) != batch.as_ref().map(|b| b.token) {
                if !batch.as_ref().is_some_and(|b| b.refined) {
                    self.predictions = None;
                }
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
                    .is_some_and(|b| b.words.iter().any(Option::is_some))
            {
                self.scan.nav.reset();
                self.scan.reset_clock();
                self.prefer_predictions = false;
            }
        }
    }
    fn retry_predictions(&mut self) {
        self.prediction_failed = false;
        self.prediction_loading = true;
        self.predictions = None;
        self.queued_predictions = None;
        self.rebuild_rows();
    }
    fn disabled(&self) -> bool {
        if !self.prediction_row_active() || self.scan.nav.escaping() {
            return false;
        }
        // The highlighted slots: one, a group of the row, or the whole row.
        // A tile the user added beside the suggestions is never disabled,
        // and a row holding one is always worth stopping at.
        let slots = crate::prediction::SLOTS;
        let Some(columns) = self.scan.highlight(&self.rows).1 else {
            return self.rows[0].len() <= slots
                && self
                    .predictions
                    .as_ref()
                    .is_none_or(|b| b.words.iter().all(Option::is_none));
        };
        if columns.end > slots {
            return false;
        }
        let columns = columns.start..columns.end.min(slots);
        self.predictions.as_ref().is_none_or(|b| {
            b.words
                .get(columns)
                .is_none_or(|slots| slots.iter().all(Option::is_none))
        })
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
    /// Carries on from the key just typed, where the user chose to stay.
    /// Suggestions then do not pull the highlight to their row.
    /// A suggestion is not stayed at: its row is about to be replaced, and
    /// new suggestions are held back while the highlight is in it.
    fn restart_after_typing(&mut self) {
        if !self.scan.stays_at_selection() || self.prediction_row_active() {
            return self.restart();
        }
        self.waiting_after_typing = false;
        self.prefer_predictions = false;
        self.scan.restart_in_place();
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
        self.layout_save_error = false;
        self.discard_stale_context(context);
        if self.scan.pending() {
            return None;
        }
        if self.scan.suspended {
            if action == Action::Select {
                if self.waiting_after_typing && !self.error {
                    self.restart_after_typing();
                } else {
                    self.error = false;
                    self.restart();
                }
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
    pub(crate) fn choose(&mut self, key: Key) -> Option<Output> {
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
    /// Shift as a suggestion should see it. An automatic Shift after a
    /// sentence end is not reported: the worker capitalises there itself.
    pub fn prediction_shift(&self) -> crate::prediction::worker::Shift {
        use crate::prediction::worker::Shift;
        match self.modifiers[0] {
            Modifier::Off => Shift::Off,
            Modifier::Once if self.auto_shift => Shift::Off,
            Modifier::Once => Shift::Once,
            Modifier::Locked => Shift::Locked,
        }
    }
    /// Whether the next word starts a sentence, as the keyboard knows it: a
    /// sentence mark it typed, not yet followed by a letter or cancelled.
    pub fn prediction_sentence_start(&self) -> bool {
        self.capitalize_next
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
        // Colon and semicolon stay literal for times, links and code. The
        // Numbers page full stop is a decimal point.
        let supported = matches!(mark, '.' | ',' | '!' | '?');
        let prose = matches!(self.page, Page::Letters | Page::Symbols);
        (supported && prose).then_some(mark)
    }
    fn choose_with_context(&mut self, key: Key, context: Option<TypingContext>) -> Option<Output> {
        self.discard_stale_context(context);
        match key {
            Key::RetryPrediction if self.prediction_failed => {
                self.retry_predictions();
                return Some(Output::RetryPrediction);
            }
            Key::RetryPrediction => return None,
            Key::Prediction(index) => {
                let batch = self.predictions.as_ref()?;
                if batch.words.get(index).is_none_or(Option::is_none) {
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
            Key::Layout => {
                return Some(Output::Layout(match self.layout {
                    crate::point_scan::KeyboardLayout::Qwerty => {
                        crate::point_scan::KeyboardLayout::SimpleQwerty
                    }
                    crate::point_scan::KeyboardLayout::SimpleQwerty => {
                        crate::point_scan::KeyboardLayout::CommonLetters
                    }
                    crate::point_scan::KeyboardLayout::CommonLetters => {
                        crate::point_scan::KeyboardLayout::Qwerty
                    }
                }));
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
            Key::Extra(..) | Key::Column(..) => {
                let action = self.cell(key)?.action.clone();
                match action {
                    PanelAction::Pause => {
                        self.scan.wait();
                        return None;
                    }
                    PanelAction::Reverse => {
                        let _ = self.scan.handle(Action::Reverse);
                        return None;
                    }
                    _ => {}
                }
                if crate::panel_extras::leaves(&action) {
                    return Some(Output::Tile(key));
                }
                // Typed or run like a key: the scan waits for the result.
                self.predictions = None;
                self.queued_predictions = None;
                self.activation = self.scan.begin_activation();
                self.activation?;
                self.pending_typed = Some(PendingTyped::Other);
                return Some(Output::Tile(key));
            }
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
            self.continue_after_typing();
        }
        completed
    }
    fn continue_after_typing(&mut self) {
        self.restart_after_typing();
        if self.wait_after_typing && self.scan.options.automatic {
            self.waiting_after_typing = true;
            self.prefer_predictions = false;
            self.scan.suspended = true;
        }
    }
    /// The keyboard has opened. It waits for Select first if the user chose
    /// to wait after a selection.
    pub fn opened(&mut self) {
        if self.wait_after_typing && self.scan.options.automatic {
            self.scan.wait();
        }
    }
    /// A repeating key was stopped by a switch. Scanning carries on as it
    /// does after typing.
    pub fn repeat_stopped(&mut self) {
        self.continue_after_typing();
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
    /// Typing continues in another window. What was typed and suggested
    /// stays behind; the keys latched and the highlight do not change.
    pub fn typing_moved(&mut self) {
        self.owned_space = None;
        self.clear_auto_capital();
        self.pending_typed = None;
        self.predictions(None, self.prediction_failed);
        // A suggestion being typed was abandoned with its window.
        if self.activation.take().is_some() {
            self.restart();
        }
    }
    #[cfg(test)]
    fn reset_context(&mut self) {
        self.modifiers = [Modifier::Off; 4];
        self.caps = false;
        self.typing_moved();
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
    /// Key glyphs drawn at the letter size. Shift is not here: its state text
    /// joins the glyph, and the key must not change size between states.
    pub const LARGE_GLYPHS: [&'static str; 8] = [
        "\u{232B}", "\u{21A9}", "\u{21E5}", "\u{2326}", "↑", "↓", "←", "→",
    ];
    /// The text drawn on a key. Utility keys with a standard symbol show the
    /// glyph; `label` keeps the readable name for the status line.
    fn display(&self, key: Key) -> String {
        match key {
            Key::Named("Backspace") => "\u{232B}".into(),
            Key::Named("Enter") => "\u{21A9}".into(),
            Key::Named("Tab") => "\u{21E5}".into(),
            Key::Named("Delete") => "\u{2326}".into(),
            // The suffix is shared with `label`, so tile and status agree.
            Key::Modifier(0) => format!("{}{}", self.shift_glyph(), self.modifiers[0].suffix()),
            // Mac keyboards and menus mark these keys with symbols. The short
            // word keeps the symbol and its name on one line of the key.
            Key::Modifier(i @ 1..=3) if self.mac => format!(
                "{}{}",
                ["\u{2303} Ctrl", "\u{2325} Opt", "\u{2318} Cmd"][i - 1],
                self.modifiers[i].suffix()
            ),
            _ => self.label(key),
        }
    }
    /// macOS shows the Apple Shift symbol, which the system font draws well.
    /// Segoe UI Symbol draws that symbol thin and small, so Windows uses the
    /// heavy upwards arrow instead.
    fn shift_glyph(&self) -> &'static str {
        if self.mac {
            "\u{21E7}"
        } else {
            "\u{2B06}"
        }
    }
    fn label(&self, key: Key) -> String {
        match key {
            Key::Prediction(i) => self
                .predictions
                .as_ref()
                .and_then(|b| b.words.get(i))
                .and_then(Option::as_ref)
                .cloned()
                .unwrap_or_default(),
            Key::RetryPrediction => "Retry predictions".into(),
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
                    Page::Symbols => "Symbols",
                },
                if page == self.page { " •" } else { "" }
            ),
            Key::Layout => match self.layout {
                crate::point_scan::KeyboardLayout::Qwerty => "Use simple QWERTY",
                crate::point_scan::KeyboardLayout::SimpleQwerty => "Use common letters",
                crate::point_scan::KeyboardLayout::CommonLetters => "Use QWERTY",
            }
            .into(),
            Key::Dock => crate::scan_panel::POSITION_PAGE.into(),
            Key::Position(dock) => crate::scan_panel::position_label(dock, self.dock),
            Key::Back => "Back".into(),
            Key::Close => self
                .close_to
                .map_or("Close keyboard", CloseTo::label)
                .into(),
            Key::Extra(..) | Key::Column(..) => self
                .cell(key)
                .map(|cell| cell.label.clone())
                .unwrap_or_default(),
        }
    }
    fn weight(&self, key: Key) -> f64 {
        match key {
            Key::Extra(row, column) => self.extra_weight(row, column),
            Key::Column(column, _) => self.column_width(column),
            Key::Character(' ', _) => 4.0,
            Key::Named("Backspace" | "Enter") => 1.9,
            Key::Named("Tab") | Key::Caps | Key::Modifier(_) => 1.6,
            Key::Close | Key::Dock | Key::Layout | Key::RetryPrediction => 1.5,
            // Room for "Navigation" on the narrow Numbers page.
            Key::Page(_) => 1.3,
            Key::Prediction(_) => self.prediction_weight(),
            _ => 1.0,
        }
    }
    /// Without columns every row stretches across the keyboard, so slots
    /// share it equally. With columns, rows keep their natural widths, so
    /// the slots widen to span the widest row of keys instead of leaving a
    /// blank beside them.
    fn prediction_weight(&self) -> f64 {
        if self.drawn_columns() == 0 {
            return 1.0;
        }
        let slots = crate::prediction::SLOTS as f64;
        let gaps = (slots - 1.0) * crate::scan_panel::KEY_GAP / 68.0;
        ((self.widest_keys() - gaps) / slots).max(1.0)
    }
    fn style(&self, key: Key, row_scan: bool) -> TileStyle {
        TileStyle {
            role: match key {
                Key::Extra(..) | Key::Column(..) => {
                    self.cell(key).map_or(TileRole::Utility, |cell| {
                        crate::panel_extras::role(&cell.action)
                    })
                }
                Key::Character(..) => TileRole::Character,
                Key::Page(_)
                | Key::Layout
                | Key::Dock
                | Key::Back
                | Key::Close
                | Key::RetryPrediction => TileRole::Toolbar,
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
        self.frame_repeating(screen, units, color, None)
    }
    /// Marks the repeating key as active and labels it, while hiding the scan
    /// highlight so it cannot suggest that Select will choose another key.
    pub fn frame_repeating(
        &self,
        screen: Rect,
        units: f64,
        color: ScannerColor,
        repeating: Option<Key>,
    ) -> Frame {
        let escaping = self.scan.nav.escaping();
        let highlight_row = self.scan.row_scan() || self.scan.group_scan() || escaping;
        let message = if self.error {
            Some("Input failed · Select to try again".to_owned())
        } else if self.layout_save_error {
            Some("Could not save layout".to_owned())
        } else {
            None
        };
        crate::scan_panel::Panel {
            key_width: if self.positioning { 128.0 } else { 68.0 },
            size: self.scan.options.panel_size,
            rows: self
                .rows
                .iter()
                .map(|row| {
                    let tile = |key: Key| {
                        let style = self.style(key, highlight_row);
                        crate::scan_panel::PanelKey {
                            text: if repeating == Some(key) {
                                format!("{}\nRepeating", self.display(key))
                            } else {
                                self.display(key)
                            },
                            weight: self.weight(key),
                            role: style.role,
                            active: style.active || repeating == Some(key),
                            blank: false,
                            // An empty suggestion slot keeps its place
                            // so the others do not move under the scan.
                            hidden: matches!(key, Key::Prediction(_)) && self.label(key).is_empty(),
                        }
                    };
                    let mut keys: Vec<crate::scan_panel::PanelKey> = row
                        .iter()
                        .filter(|key| !matches!(key, Key::Column(..)))
                        .map(|&key| tile(key))
                        .collect();
                    keys.extend(self.row_filler(row));
                    // Every row carries a slot for each column, so a column
                    // lines up where a row has no tile in it.
                    for column in 0..self.drawn_columns() {
                        let found = row
                            .iter()
                            .find(|key| matches!(key, Key::Column(c, _) if *c == column));
                        keys.push(match found {
                            Some(&key) => tile(key),
                            None => {
                                let mut blank = crate::scan_panel::PanelKey::blank();
                                blank.weight = self.column_width(column);
                                blank
                            }
                        });
                    }
                    keys
                })
                .collect(),
            message,
            dock: self.dock,
            selected: (repeating.is_none() && !self.scan.suspended && !self.disabled())
                .then(|| self.scan.highlight(&self.rows)),
            row_scan: highlight_row,
            thickness: Default::default(),
            appearance: self.scan.options.panel_theme.appearance(),
        }
        .frame(screen, units, color)
    }
}

#[cfg(test)]
mod tests;
