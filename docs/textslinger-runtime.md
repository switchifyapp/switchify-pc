# Bundled TextSlinger runtime

Switchify PC is moving its word suggestions to the n-gram model from
[TextSlinger](https://github.com/kdv123/textslinger) (tracking issue #1427). The app
bundles a self-contained runtime for it. In this release the runtime is bundled,
signed and checked, but the app does not use it yet; #1423 switches the keyboard to it.

## Where it comes from

[switchify-prediction](https://github.com/switchifyapp/switchify-prediction) builds
the runtime for each platform and publishes it with each release. See its
`docs/textslinger-runtime.md` for its contents and build. `scripts/textslinger-runtime.json`
pins the release and each archive's size and SHA-256.

`npm run prediction-model` runs `scripts/fetch-textslinger-runtime.mjs`, which:

- downloads the archive for the build target (`TAURI_ENV_TARGET_TRIPLE`, or rustc's host);
- checks its size and hash and that every path stays inside the runtime;
- unpacks it to `src-tauri/resources/textslinger/runtime`, replacing any earlier copy all at once and leaving out static libraries (`.a`, `.lib`) that nothing loads;
- records the archive's hash in `runtime.sha256`, so later runs skip the download.

Windows ARM64 bundles the Windows x64 runtime and runs it under Windows' x64
emulation, because there is no Python 3.10 or numpy 1.26 build for ARM64. macOS
releases are Apple Silicon only. On Linux the fetch is skipped, since Switchify PC is not packaged there.

## The bridge

`src-tauri/resources/textslinger/bridge.py` is the helper the app will run with the
bundled Python: `python -I -B bridge.py --runtime <runtime folder>`. It reads the
model path and alphabet from the runtime's `manifest.json`. It loads the model, sends
`{"ready": true}`, then answers framed JSON requests (a 4-byte little-endian length,
then at most 16 KiB of JSON) over private pipes:

- `{"id": n, "before": text, "prefix": word}` gets `{"id": n, "words": [...]}`, with up to five lowercase completions;
- `{"shutdown": true}` gets `{"stopped": true}`.

It never opens a network connection, and never logs, saves or prints the text it
receives. It waits while idle with no time limit. A request that takes more than
30 seconds makes it exit, so the app can report suggestions as unavailable.

## Packaging and signing

`src-tauri/tauri.conf.json` bundles `resources/textslinger/bridge.py` and
`resources/textslinger/runtime/**/*`.

- **macOS release:** `scripts/sign-macos-textslinger-runtime.sh` signs every Mach-O file in the runtime with the Developer ID, hardened runtime and a secure timestamp before `tauri build`. The release then runs the signed runtime once. `scripts/verify-macos-release.sh` already rejects any nested binary not signed with the release identity.
- **Windows release:** `scripts/Build-WindowsUiAccess.ps1` signs every `.exe`, `.dll` and `.pyd` in the runtime that isn't already validly signed, such as Microsoft's C runtime, because Smart App Control can block unsigned DLLs. `scripts/Verify-WindowsUiAccessPackage.ps1` checks every one in the installer and runs the installed runtime once. CI runs the same script with unsigned files allowed.

## Checks

- `scripts/textslinger-bridge-test.py`: dependency-free bridge tests. CI runs them with the bundled Python.
- `scripts/check-textslinger-runtime.mjs <folder>`: finds `textslinger/runtime` and `bridge.py` under a folder, runs them once with fixed text, and requires "door" for "please open the ". CI runs it after fetching, and on the packaged app on macOS, Windows x64 and Windows ARM64. The ARM64 run covers x64 emulation.
- `scripts/fetch-textslinger-runtime.node-test.mjs`: pins, target mapping, archive checks and the all-at-once replacement.

An installer update overwrites files but may not remove ones a newer runtime no
longer ships. Check this when the runtime version first changes.

To update the runtime, publish a switchify-prediction release, then update
`scripts/textslinger-runtime.json` with its asset URLs, sizes and SHA-256 digests.
