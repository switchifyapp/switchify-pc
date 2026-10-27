//! Light and dark colours for the native panels: the keyboard, the mouse
//! panel, Home and the action menus. Point scanning lines, pointer markers
//! and the floating prompt keep their own colours.
use crate::scanning::ScannerColor;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the computer is using a light appearance. Set from the main
/// window's theme at startup and whenever the system setting changes;
/// read when a panel frame is built, so panels follow the change on their
/// next tick without being restarted.
static SYSTEM_LIGHT: AtomicBool = AtomicBool::new(false);

pub fn set_system_light(light: bool) {
    SYSTEM_LIGHT.store(light, Ordering::Relaxed);
}
pub fn system_light() -> bool {
    SYSTEM_LIGHT.load(Ordering::Relaxed)
}

/// The saved choice. `System` follows the computer's light or dark setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelTheme {
    #[default]
    System,
    Light,
    Dark,
}
impl PanelTheme {
    pub fn appearance(self) -> Appearance {
        self.appearance_when(system_light())
    }
    /// Pure, so tests never have to touch the shared system flag.
    pub fn appearance_when(self, system_light: bool) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System if system_light => Appearance::Light,
            Self::System => Appearance::Dark,
        }
    }
}

/// What a panel is drawn with once the system setting has been consulted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}
impl Appearance {
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                panel: [20, 24, 32],
                band: [30, 36, 48],
                character: [43, 51, 66],
                utility: [34, 41, 54],
                menu_bar: [52, 61, 79],
                other: [25, 30, 40],
                border: [62, 72, 89],
                band_border: [74, 86, 108],
                text: [255, 255, 255],
                highlight_base: [30, 35, 46],
            },
            Self::Light => Palette {
                panel: [240, 242, 246],
                band: [226, 230, 237],
                character: [255, 255, 255],
                utility: [236, 239, 244],
                menu_bar: [214, 219, 228],
                other: [244, 246, 249],
                border: [196, 203, 214],
                band_border: [176, 185, 200],
                text: [20, 24, 32],
                highlight_base: [255, 255, 255],
            },
        }
    }
    /// The scanner colour as drawn on panels of this appearance. Light
    /// panels use a deeper shade so every choice stays visible; white
    /// becomes dark grey.
    pub fn highlight(self, color: ScannerColor) -> [u8; 3] {
        match self {
            Self::Dark => color.rgb(),
            Self::Light => match color {
                ScannerColor::Red => [198, 40, 40],
                ScannerColor::Green => [27, 140, 60],
                ScannerColor::Blue => [25, 103, 210],
                ScannerColor::Yellow => [176, 112, 0],
                ScannerColor::White => [58, 64, 76],
            },
        }
    }
    /// The fill of a highlighted key: the panel's base tinted with the highlight.
    pub fn highlight_fill(self, color: ScannerColor) -> [u8; 3] {
        let base = self.palette().highlight_base.map(u16::from);
        let highlight = self.highlight(color).map(u16::from);
        std::array::from_fn(|i| ((base[i] * 3 + highlight[i]) / 4) as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub panel: [u8; 3],
    /// The menu bar band along the top of every panel.
    pub band: [u8; 3],
    pub character: [u8; 3],
    pub utility: [u8; 3],
    pub menu_bar: [u8; 3],
    pub other: [u8; 3],
    pub border: [u8; 3],
    pub band_border: [u8; 3],
    pub text: [u8; 3],
    highlight_base: [u8; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_highlights_are_the_scanner_colours_and_light_ones_are_deeper() {
        for color in [
            ScannerColor::Red,
            ScannerColor::Green,
            ScannerColor::Blue,
            ScannerColor::Yellow,
            ScannerColor::White,
        ] {
            assert_eq!(Appearance::Dark.highlight(color), color.rgb());
            let [r, g, b] = color.rgb().map(u16::from);
            assert_eq!(
                Appearance::Dark.highlight_fill(color),
                [(90 + r) / 4, (105 + g) / 4, (138 + b) / 4].map(|c| c as u8)
            );
            let light = Appearance::Light.highlight(color);
            let luma =
                |[r, g, b]: [u8; 3]| u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
            // Visible against a white key and darker than the dark-panel shade.
            assert!(luma(light) < luma([200, 200, 200]), "{color:?}");
            assert!(luma(light) < luma(color.rgb()), "{color:?}");
            assert_ne!(Appearance::Light.highlight_fill(color), [255, 255, 255]);
        }
    }

    #[test]
    fn the_chosen_theme_wins_and_system_follows_the_computer() {
        assert_eq!(PanelTheme::Light.appearance(), Appearance::Light);
        assert_eq!(PanelTheme::Dark.appearance(), Appearance::Dark);
        assert_eq!(PanelTheme::default(), PanelTheme::System);
        assert_eq!(PanelTheme::System.appearance_when(true), Appearance::Light);
        assert_eq!(PanelTheme::System.appearance_when(false), Appearance::Dark);
        assert_eq!(PanelTheme::Dark.appearance_when(true), Appearance::Dark);
        assert_eq!(PanelTheme::Light.appearance_when(false), Appearance::Light);
    }
}
