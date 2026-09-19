# QWERTY scanning keyboard

Switchify has its own UK English keyboard on Windows and macOS. It uses the saved local switches or the existing Android remote scanning actions. It does not use the operating system's on-screen keyboard.

Choose a point and select **Type here** to click that point once and open the keyboard. To keep the existing input focus without clicking, select **More → Keyboard**. The keyboard does not take focus or accept mouse clicks.

Scan a row, select it, then scan and select a key. The escape slot returns to rows. After typing, scanning starts again at the first row. Existing automatic/manual movement, speed, pause, reverse and inactivity suspension apply. The keyboard stays open after Space, Backspace, Enter and Tab. **Close** returns to point scanning.

Pages:

- **Letters:** UK QWERTY and punctuation, with Backspace beside the top letter row.
- **Functions:** Esc, F1–F12, arrows, Home, End, Page Up, Page Down and Delete. Windows also has Insert, Print Screen, Scroll Lock and Pause.
- **Numbers:** digits, decimal point, arithmetic operators, UK number-row symbols (including £), Enter and Backspace. Digits are independent of hardware Num Lock.

The bottom control row starts with Close keyboard, followed by Letters, Navigation, Numbers and docking. Space has a dedicated wide key. Editing and modifier keys are wider than ordinary characters; duplicate Shift and Caps controls are removed from the letters page. A header names the current page and scan target. The return-to-rows slot highlights only the header. Active modifiers and the current page have an indicator separate from the scan highlight. Modifiers are available on every page. Shift, Ctrl, Alt/Option and Windows/Command cycle through **off → once → locked → off**. The label shows once or locked. Caps is a keyboard-local uppercase setting; Shift reverses its letter case. Page and position controls do not consume a one-shot modifier.

Modifier keys are pressed only around each emitted shortcut and immediately released. Selecting a locked modifier does not hold that operating-system key while the scanner runs. Ordinary characters use text injection; command combinations and navigation use native key events. Native shortcuts retain the operating system's layout semantics.

The keyboard closes when the foreground target or display environment changes, or its scan session ends. Failed input clears keyboard modifiers and requires Select to resume. Stop, disconnect and application exit use the shared deterministic input cleanup. Word prediction can be enabled in Scanning settings. Five suggestions appear on the Letters page. Selecting a suggestion inserts its missing suffix and a space. Prediction uses local, read-only data; no personal vocabulary is saved. See [word prediction](word-prediction.md) for context availability and validation.

## Manual validation

Use synthetic text in Notepad and a browser on Windows, and TextEdit and a browser on macOS. Launch macOS through `npm run macos:run` to retain its Accessibility identity.

1. Use Type here and verify exactly one click focuses the intended field. Use More → Keyboard and verify no click or focus change occurs.
2. Type lowercase letters, uppercase letters with Shift/Caps, and UK punctuation including £, @ and double quotes. Test Space, Backspace, Enter and Tab.
3. Test Ctrl+A/C/V on Windows and Command+A/C/V on macOS. Cycle once/locked/off, including Shift with another modifier, and confirm no modifier remains physically held between selections.
4. Visit every page. Verify row/key highlighting, reverse movement, row escape, suspension/resume, returning to the first row after a key and Close.
5. Move the keyboard between the top and bottom. Check a scaled display and a secondary display, including negative coordinates. Verify the keyboard fits the work area and never activates its native windows.
6. Switch foreground apps, disconnect remote scanning, change display configuration and close Switchify. Verify overlays disappear and owned input is released.

Automated tests use fake input adapters only. Native manual results must be recorded separately; compilation and unit tests do not establish live application compatibility.
