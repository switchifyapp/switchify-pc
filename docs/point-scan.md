# Native point scan

Point scan ports the Android line-only and grid-then-line techniques to Switchify PC. The reference source is `switchifyapp/switchify-android` commit `856720d8747e2f3d1724a572bf754ffae05df299`, especially `PointScanLineManager`, `PointScanBlockManager`, and `ContinuousLineSpeedUtils`.

Open **Settings → Switches** to add named keyboard switches and assign normal and hold actions. There is no on/off control: scanning is armed whenever the saved switches cover the current mode (Select for automatic scanning; Select, Next and Previous for manual) and the environment allows it. The runtime re-arms after a save, after key learning, after Escape, when an Android session ends, and retries a failed key reservation every two seconds. Fresh installs have no assignments; old point-scan keys migrate once. See [switch assignments](switches.md). Scanning arms at startup, so assigned keys are reserved from launch.

Focus the intended application and press Select to start. Line mode chooses X, then Y, and opens an action menu. Grid mode chooses a row, then a cell, before the same line sequence. Selecting happens on switch release; holding any switch freezes the position and repeat keydowns do not select again. The scan resets after a menu click or completed drag and waits for the next Select. Movement wraps at the selected region's edges. Automatic movement stops after three full passes of the current phase without a selection; the scan resets and waits for the next Select. Manual steps never trigger this limit, and each Select starts a fresh count for the next phase. Android's five speeds are 45, 75, 120, 180, and 270 logical units per second, with delayed ticks capped at 250 ms.

The scan uses the monitor under the pointer when it starts. Windows uses native physical coordinates and display scaling; macOS uses Core Graphics display units and converts overlay rectangles to AppKit coordinates. A monitor geometry change cancels scanning. Native overlay strips are topmost, click-through, and nonactivating. The pointer moves only when a menu action executes.

This first port uses switches attached to the computer. An Android connection pauses local scanning, which resumes when the connection ends. Existing authenticated Bluetooth commands and switch-forwarding profiles are unchanged. Input permission is still required on macOS. Point settings use a separate `point-scan.json` in the existing Tauri application configuration directory, so old settings and pairing schemas remain unchanged.

Changes save automatically, including when you leave Settings, and apply straight away: saving pauses scanning and the runtime re-arms it with the new settings. A failed save keeps your edits and offers Retry save. Navigating away does not stop scanning.

The engine is independent of OS input. The action executor uses the existing `InputInjector` adapter. Automated tests use a fake adapter and never move the real pointer. Switchify's local `switch_input` adapter supplies press/release events on both platforms. See `switches.md` for Windows capture limitations. Only the main window can configure scanning through IPC.

Manual validation should cover both modes, every speed, manual movement, holding and releasing switches, Escape, focus retention, display changes, mixed scaling, a Bluetooth connection during scanning, and application exit. Run macOS input checks through `npm run macos:run` to retain the stable Accessibility identity. No physical switch or real desktop input is exercised by automated tests.

## Reusing scanning in Switchify PC

`scanning.rs` is the pure shared core. `Session<T>` owns start, pause, completion, cancellation and bounded automatic timing. The shared gesture engine turns physical presses and releases into normal and hold actions; the controller rejects stale capture generations. `scan_tree::Navigator` and `Interval` provide traversal and timing. A technique implements `start`, `advance`, `handle`, `reset`, `frame` and `phase`, with its own typed selection result. The test-only item technique exercises this contract without pointer coordinates or desktop input.

`point_scan.rs` implements row/cell and X/Y selection. Its engine receives only point settings; the existing flat `Config` maps into shared switch settings and point settings. `scanning_runtime.rs` owns embedded switch dispatch, session ticking, persistence, event publication and shutdown. Its `Adapter` supplies configuration, environment validation and selection activation. `scan_host.rs` renders shared frame strips through nonactivating Windows and macOS windows. `point_scan_runtime.rs` supplies the display and click adapters, and `point_scan_activation.rs` checks input state before clicking through `InputInjector`.

Only one local technique is installed at a time. A future technique can use the shared controller with its own adapter; adding a technique chooser is separate work. Transport cleanup calls the shared scan service, which invalidates callbacks immediately and releases native resources on the main thread. Queued cancellation cannot stop a newer registration.

The existing `point-scan.json`, `get_point_scan`, `configure_point_scan` and `point-scan-changed` retain their field names and payload shapes. No production item scanner, UI Automation scanner or keyboard integration is added here.

## Grid row escape

Grid rows and cells use the shared pure Rust `scan_tree` navigator. After the last cell, or before the first when moving backwards, the scan outlines the row and displays **Back to rows** for one block interval. Select returns to choosing rows with the same row highlighted and forward movement restored. Passing escape without selecting repeats the cells in the current direction and counts one automatic cycle. The escape remains available on the third pass before scanning resets. Manual steps reset the block interval and never exhaust the scan.

The navigator supports nested branches and typed leaves for future item scanning. It removes empty branches and collapses single-child branches. Root traversal wraps without an escape slot. Grid leaf selection still begins the existing X/Y precision scan. No desktop accessibility discovery is included.

The shared frame carries optional label metadata, rendered by a separate nonactivating, click-through native host on the scan display. The technique label stays visible while a switch is held, until a hold-action prompt takes priority. Reset, Escape, configuration changes, Android connection and shutdown clear it through session cleanup. The desktop view adds the `rowEscape` phase; saved settings and Bluetooth interfaces are unchanged.

Physical validation should include forward/reverse escape with a real switch, pause/hold behavior, the final automatic cycle, mixed-DPI label placement, focus retention, disconnect and exit on both Windows and macOS. Automated tests use fake input only.

## Actions at a point

Choosing a point opens a native grid beside its marker. The rows are Left click / Right click / Double click; Scroll / Drag; Choose another point / Cancel. The shared tree navigator handles row/item selection and row escape. The menu uses the current automatic/manual mode and block interval. After three automatic passes, it retains the target and shows Select to resume. That Select resumes scanning without executing an action.

Scroll offers Up / Down, Left / Right, and Back to actions. Each selection moves the pointer to the target and sends three wheel steps; the selected direction remains available. Back restores the main menu's Scroll item. Choose another point immediately restarts the configured technique on the original display. Clicks and completed drags return to armed idle.

Drag retains the source and scans a destination on the same display without holding a button. The confirmation menu offers Drag here, Choose destination again and Cancel drag. Cancel drag returns to the source menu. Confirmation moves to the source, presses the left button, interpolates to the destination over 300 ms on runtime ticks, and releases. No blocking sleep is used. Ordinary selections cannot issue another action during execution. Failed releases stay owned and cleanup retries before rearming and on disabled ticks.

`point_workflow` separates point selection, menu navigation and typed action requests. Its current selection policy always opens the menu; a future auto-select policy can reuse `default_click` without changing the executor. No auto-select timer or setting exists. The reusable `scan_menu` defines stable action IDs and rows. Shared frames carry menu tiles, labels and highlight strips; all native windows remain click-through and nonactivating.

The desktop view adds menu, menuSuspended, dragDestination, dragConfirmation and executing phases. Existing point phases, saved settings and Bluetooth commands retain their shapes. Foreground changes, display changes, Android connections, switch editing/learning and shutdown cancel the workflow. Targets and foreground identity remain in memory and are never logged or emitted. Automated action tests use fake input only; physical Windows/macOS focus, scaling, target, scrolling and drag checks remain required for hardware qualification.

The action menu uses a fixed grid of square icon tiles with labels beneath the artwork. A yellow border and amber background identify the current row or item. The same artwork is drawn on Windows and macOS.
