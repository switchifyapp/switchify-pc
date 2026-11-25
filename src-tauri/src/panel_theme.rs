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

/// A colour the user gave one of their tiles. A fixed set, so each has a
/// shade for dark panels and one for light panels that keeps the label and
/// the scan highlight readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TileColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Grey,
}
impl TileColor {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 8] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
        Self::Pink,
        Self::Grey,
    ];
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
    /// The face of a tile the user coloured. The label keeps the palette's
    /// text colour, so dark panels use deep shades and light panels pale ones.
    pub fn tile_fill(self, color: TileColor) -> [u8; 3] {
        match self {
            Self::Dark => match color {
                TileColor::Red => [150, 40, 40],
                TileColor::Orange => [156, 84, 20],
                TileColor::Yellow => [130, 106, 10],
                TileColor::Green => [30, 112, 60],
                TileColor::Blue => [28, 90, 160],
                TileColor::Purple => [104, 62, 160],
                TileColor::Pink => [156, 48, 110],
                TileColor::Grey => [104, 112, 126],
            },
            Self::Light => match color {
                TileColor::Red => [255, 170, 165],
                TileColor::Orange => [255, 196, 130],
                TileColor::Yellow => [250, 228, 120],
                TileColor::Green => [160, 225, 170],
                TileColor::Blue => [160, 200, 255],
                TileColor::Purple => [208, 184, 255],
                TileColor::Pink => [255, 180, 220],
                TileColor::Grey => [176, 183, 194],
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
    fn tile_colours_keep_the_label_readable_and_never_pass_for_the_highlight() {
        // WCAG relative luminance and contrast ratio.
        let luminance = |rgb: [u8; 3]| {
            let channel = |c: u8| {
                let c = f64::from(c) / 255.0;
                if c <= 0.03928 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
        };
        let contrast = |a: [u8; 3], b: [u8; 3]| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        let distance = |a: [u8; 3], b: [u8; 3]| -> u32 {
            (0..3).map(|i| u32::from(a[i].abs_diff(b[i]))).sum()
        };
        for appearance in [Appearance::Dark, Appearance::Light] {
            let palette = appearance.palette();
            let fills = TileColor::ALL.map(|color| appearance.tile_fill(color));
            for (index, fill) in fills.iter().enumerate() {
                let color = TileColor::ALL[index];
                assert!(
                    contrast(palette.text, *fill) >= 4.5,
                    "{appearance:?} {color:?}"
                );
                // Told apart from an uncoloured tile and from every other colour.
                for plain in [palette.utility, palette.other, palette.panel] {
                    assert!(distance(*fill, plain) >= 40, "{appearance:?} {color:?}");
                }
                for other in &fills[index + 1..] {
                    assert!(distance(*fill, *other) >= 40, "{appearance:?} {color:?}");
                }
                // A coloured tile never looks like a highlighted one.
                for scanner in [
                    ScannerColor::Red,
                    ScannerColor::Green,
                    ScannerColor::Blue,
                    ScannerColor::Yellow,
                    ScannerColor::White,
                ] {
                    assert!(
                        distance(*fill, appearance.highlight_fill(scanner)) >= 40,
                        "{appearance:?} {color:?} {scanner:?}"
                    );
                }
            }
        }
        assert_eq!(
            serde_json::to_value(TileColor::ALL).unwrap(),
            serde_json::json!([
                "red", "orange", "yellow", "green", "blue", "purple", "pink", "grey"
            ])
        );
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
