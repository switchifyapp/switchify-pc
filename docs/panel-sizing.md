# Scanning panel sizing

One shared Panel size preference in scanning Appearance controls mouse, keyboard, Home and action menus. Small, Medium and Large use 0.8, 1.0 and 1.2. Missing or unknown saved values use Medium; per-area overrides are not supported. Point lines, pointer rings and application settings windows are unaffected.

Mouse keys use 128 logical pixels per weight unit; keyboard character pages use 68. The keyboard Position page uses 128 for its text labels. Rows use 56-pixel controls, 8-pixel gaps, 16-pixel outer padding and a 48-pixel status area. Width comes from the widest weighted row, including reserved blanks. Medium mouse Movement is 568 by 392; keyboard dimensions depend on layout and whether its eight prediction slots are enabled. Menus keep their existing 180-pixel grid pitch at Medium.

The preset and display scale are applied once to all geometry and text. A uniform fit factor keeps the complete panel within its display bounds. Status text, highlights and prediction contents cannot resize a panel. Page/layout changes may resize it. Docking, relocation and pointer avoidance use measured bounds.

## Validation

Geometry tests cover presets, fractional DPI, small and negative-coordinate displays, nine docks, directional alignment, all menus and stable keyboard prediction bounds. Existing fake-input scanning and prediction acceptance tests remain in the suite. UI and serialization tests cover old settings and the shared control.

On Windows, export synthetic images with the actual GDI text renderer, without displaying a window or injecting input:

```powershell
$env:SWITCHIFY_PANEL_FIXTURES = Join-Path $PWD '.cache/panel-fixtures'
cargo test --manifest-path src-tauri/Cargo.toml export_panel_size_fixtures -- --ignored --nocapture
```

Each panel is exported twice: dark as `name-Size.png` and light as `name-Size-light.png`, so the Panel theme palettes can be checked side by side. `keyboard-predictions-*.png` fills the suggestion row with a mix of filled slots, empty placeholder outlines and words long enough to shrink or split with a hyphen.

```powershell
# Example: .cache/panel-fixtures/mouse-Medium.png and mouse-Medium-light.png
```

Windows Small and Medium mouse and Medium keyboard fixtures were visually checked for labels, spacing, direction alignment and reserved prediction slots. macOS visual inspection is not available on this Windows machine; cross-platform compilation and tests are covered by CI. This change does not modify platform focus or click-through behavior.
