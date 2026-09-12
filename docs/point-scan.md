# Native point scan

Point scan ports the Android line-only and grid-then-line techniques to Switchify PC. The reference source is `switchifyapp/switchify-android` commit `856720d8747e2f3d1724a572bf754ffae05df299`, especially `PointScanLineManager`, `PointScanBlockManager`, and `ContinuousLineSpeedUtils`.

Open **Point scan**, choose automatic or manual scanning, and enable it. Local keyboard-emulating switch interfaces can use Space to select, Enter to step forward, Backspace to step backward, and F8 to pause or resume. The four keys can be changed. Escape always disables scanning and releases the reserved keys. Scanning is off at every application startup.

Focus the intended application and press Select to start. Line mode chooses X, then Y, and clicks once. Grid mode chooses a row, then a cell, before the same line sequence. Selecting happens on switch release; holding Select freezes the position and repeat keydowns do not select again. The scan resets after a click and waits for the next Select. Movement wraps at the selected region's edges. Android's five speeds are 45, 75, 120, 180, and 270 logical units per second, with delayed ticks capped at 250 ms.

The scan uses the monitor under the pointer when it starts. Windows uses native physical coordinates and display scaling; macOS uses Core Graphics display units and converts overlay rectangles to AppKit coordinates. A monitor geometry change cancels scanning. Native overlay strips are topmost, click-through, and nonactivating. The pointer moves only for the final click.

This first port uses switches attached to the computer. Android must be disconnected before enabling it, and an Android connection cancels local scanning. Existing authenticated Bluetooth commands and switch-forwarding profiles are unchanged. Input permission is still required on macOS. Point settings use a separate `point-scan.json` in the existing Tauri application configuration directory, so old settings and pairing schemas remain unchanged.

The engine is independent of OS input. Activation uses the existing `InputInjector` adapter. Automated tests use a fake adapter and never move the real pointer. The Tauri global-shortcut plugin supplies global press/release events on both platforms; the existing dependencies did not include a switch-key listener. Only the main window can configure scanning through IPC.

Manual validation should cover both modes, every speed, manual movement, holding and releasing switches, Escape, focus retention, display changes, mixed scaling, a Bluetooth connection during scanning, and application exit. Run macOS input checks through `npm run macos:run` to retain the stable Accessibility identity. No physical switch or real desktop input is exercised by automated tests.
