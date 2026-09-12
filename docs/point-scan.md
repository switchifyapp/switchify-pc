# Native point scan

Point scan ports the Android line-only and grid-then-line techniques to Switchify PC. The reference source is `switchifyapp/switchify-android` commit `856720d8747e2f3d1724a572bf754ffae05df299`, especially `PointScanLineManager`, `PointScanBlockManager`, and `ContinuousLineSpeedUtils`.

Open **Settings → Switches** to add named keyboard switches and assign normal and hold actions. Then enable scanning in **Settings → Scanning**. Fresh installs have no assignments; old point-scan keys migrate once. See [switch assignments](switches.md). Scanning stays off at startup.

Focus the intended application and press Select to start. Line mode chooses X, then Y, and clicks once. Grid mode chooses a row, then a cell, before the same line sequence. Selecting happens on switch release; holding any switch freezes the position and repeat keydowns do not select again. The scan resets after a click and waits for the next Select. Movement wraps at the selected region's edges. Android's five speeds are 45, 75, 120, 180, and 270 logical units per second, with delayed ticks capped at 250 ms.

The scan uses the monitor under the pointer when it starts. Windows uses native physical coordinates and display scaling; macOS uses Core Graphics display units and converts overlay rectangles to AppKit coordinates. A monitor geometry change cancels scanning. Native overlay strips are topmost, click-through, and nonactivating. The pointer moves only for the final click.

This first port uses switches attached to the computer. Android must be disconnected before enabling it, and an Android connection cancels local scanning. Existing authenticated Bluetooth commands and switch-forwarding profiles are unchanged. Input permission is still required on macOS. Point settings use a separate `point-scan.json` in the existing Tauri application configuration directory, so old settings and pairing schemas remain unchanged.

Changes save automatically, including when you leave Settings. Enabling waits for pending saves; a failed save keeps your edits and offers Retry save. Configuration is locked while scanning is enabled. Navigating away does not stop scanning.

The engine is independent of OS input. Activation uses the existing `InputInjector` adapter. Automated tests use a fake adapter and never move the real pointer. Embedded USAHP supplies physical press/release events and owns native capture on both platforms. Only the main window can configure scanning through IPC.

Manual validation should cover both modes, every speed, manual movement, holding and releasing switches, Escape, focus retention, display changes, mixed scaling, a Bluetooth connection during scanning, and application exit. Run macOS input checks through `npm run macos:run` to retain the stable Accessibility identity. No physical switch or real desktop input is exercised by automated tests.

## Reusing scanning in Switchify PC

`scanning.rs` is the pure shared core. `Session<T>` owns start, pause, completion, cancellation and bounded automatic timing. The shared gesture engine turns physical presses and releases into normal and hold actions; the controller rejects stale capture generations. `Cycle` and `Interval` provide traversal and timing. A technique implements `start`, `advance`, `handle`, `reset`, `frame` and `phase`, with its own typed selection result. The test-only item technique exercises this contract without pointer coordinates or desktop input.

`point_scan.rs` implements row/cell and X/Y selection. Its engine receives only point settings; the existing flat `Config` maps into shared switch settings and point settings. `scanning_runtime.rs` owns embedded switch dispatch, session ticking, persistence, event publication and shutdown. Its `Adapter` supplies configuration, environment validation and selection activation. `scan_host.rs` renders shared frame strips through nonactivating Windows and macOS windows. `point_scan_runtime.rs` supplies the display and click adapters, and `point_scan_activation.rs` checks input state before clicking through `InputInjector`.

Only one local technique is installed at a time. A future technique can use the shared controller with its own adapter; adding a technique chooser is separate work. Transport cleanup calls the shared scan service, which invalidates callbacks immediately and releases native resources on the main thread. Queued cancellation cannot stop a newer registration.

The existing `point-scan.json`, `get_point_scan`, `configure_point_scan` and `point-scan-changed` retain their field names and payload shapes. No production item scanner, UI Automation scanner or keyboard integration is added here.
