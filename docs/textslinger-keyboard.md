# Trying TextSlinger in the keyboard

Issue #1419 builds on the engine spike in PR #1418. This branch adds an explicitly
opted-in local keyboard backend. It is not a packaged installer or a new saved
setting. The `textslinger-trial` Cargo feature and
`SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND` environment variable are both required.
Regular builds ignore that variable and keep their existing predictor.

## Start on this Windows machine

From the repository root:

```powershell
./scripts/Start-TextSlingerKeyboard.ps1 -Dev
```

The defaults use the native Python environment prepared during the spike at
`$env:LOCALAPPDATA/SwitchifyTextSlingerSpike/venv/Scripts/python.exe` and the model
in the adjacent `textslinger/assets/lm_dec19_char_tiny_12gram.arpa` checkout.
Use `-Python` and `-Model` to override those paths. See `textslinger-spike.md`
for the runtime installation and model hash. No WSL is used by this launcher.

`-Dev` starts Vite and the native Tauri app together with TextSlinger enabled,
and keeps the terminal running for hot reload. Close the app and stop that
terminal command to end the development session.

Without `-Dev`, the script builds the embedded frontend and a debug executable with the trial
feature, snapshots the executable and bridge into `.cache/textslinger-keyboard`, and launches the
app with the trial command only in its inherited environment. It restores the
caller's environment afterward. It does not install or replace the regular app.
Use `-SkipBuild` to reopen that snapshot; use `-NoLaunch` to build only.
Those switches apply only to the standalone build, not `-Dev`.
Quit any running Switchify PC instance using its tray menu before launching.
The same application identity preserves your switch and keyboard settings.

Open the scanning keyboard as usual and type with it. After the model starts,
the first five word tiles use TextSlinger. Phrase tiles stay empty. Loading can
take tens of seconds, especially with a cold file cache. Keyboard interaction
does not wait for the model to load. The existing unavailable/retry control
applies if the helper fails. Quit this app and reopen the installed app to use
normal predictions again.

This trial targets ordinary desktop applications. The unsigned debug executable
does not have the installed app's Windows UIAccess signing and cannot be used
to assess interaction with elevated or protected applications.

## Behavior and safety

- The bridge supports literal English letter/apostrophe completions. It does
  not implement phrases, personal learning, noisy input, abbreviations, or
  nearby-key corrections. Unsupported prefixes return empty results without
  disabling later predictions.
- Lowercase names, lowercase `i`, non-words, and complete-word echoes are known
  limits inherited from the spike. Switchify still owns casing, context tracking,
  stale-token checks, and selection insertion. No quality claim is made.
- The model receives accepted text through private framed pipes, never command
  arguments or HTTP. Child diagnostics are discarded. Text is not logged or
  saved by this backend.
- Startup is deferred until the worker receives a request, after the Windows
  parent has assigned its kill-on-close job. Python inherits that containment.
  Existing macOS worker process-group cleanup applies on that platform, though
  this launcher and live app experience have only been tried on Windows.
- The model loads on a background thread with a 90-second startup deadline.
  Each prediction retains the 1,200 ms reply deadline and sequence checks.
  Typing and scanning continue while loading; an unavailable helper requires
  the existing retry action rather than silently switching prediction engines.
- In keyboard mode the Python bridge waits indefinitely for input, so an idle
  keyboard is not disconnected after 30 seconds. Active requests still have a
  30-second helper watchdog in addition to the Rust reply deadline. Closing the
  keyboard discards its context-holding worker through the existing lifecycle.

## Validation

The opt-in `textslinger_keyboard_loading_and_idle` test constructs the actual
keyboard worker engine and asynchronous database with fake desktop adapters.
It checks a prompt loading response, a real `coffee` suggestion, a 35-second
idle pause, then a real `door` insertion operation. No OS typing or pointer
movement is performed by automated tests. The original spike tests now share
the same adapter as the live backend.

The normal Rust tests include failed-startup and reply-validation cases. Python
transport tests additionally check matching warm-up/serving alphabets and that
keyboard mode starts its watchdog after receiving input. A clean-machine
installer, macOS app trial, Windows ARM64 runtime, and elevated-app testing
remain separate work.
