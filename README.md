# Switchify PC

Switchify PC is the Rust/Tauri desktop companion for controlling Windows and macOS from the Switchify Android app. The React/TypeScript interface and Rust backend now live at the repository root. Platform adapters provide Bluetooth LE peripheral support, authenticated pairing, input injection, overlays, profiles, startup and tray behavior, diagnostics, and update checks.

The application uses the shipping product identity `Switchify PC` and bundle identifier `com.enaboapps.switchify.pc`. Only one application may advertise the Switchify Bluetooth service at a time.

## Prerequisites

- Node.js 24
- Rust 1.97.1 through rustup, including `rustfmt` and `clippy`
- Windows: Visual Studio Build Tools with the Desktop development with C++ workload
- macOS: Xcode Command Line Tools and macOS 13 or later

## Run

Install dependencies from the repository root:

```bash
npm ci
```

For macOS Bluetooth and Accessibility testing, build and launch the signed debug app:

```bash
npm run macos:run
```

The command idempotently creates a machine-local, ten-year code-signing identity named `Switchify PC Development`, if needed. Its private key is non-extractable and no certificate, key, or password is stored in Git. Preserve the identity in the login Keychain: deleting or recreating it requires granting Accessibility again.

The first time, choose **Open Accessibility Settings**, enable **Switchify PC**, and return to the app. It silently updates to Ready when the window regains focus. If the row is already enabled but access remains required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry. The setup never resets TCC.

The signed macOS application stores Android pairing tokens in `pairing-tokens.json` in its application-data directory. The file is written atomically with user-only `0600` permissions, and its parent directory is restricted to `0700`. Windows uses its native credential store.

The promoted identity starts with new settings, a new desktop ID, and no paired devices. Data, credentials, Accessibility approval, and certificates from earlier development builds are not migrated or removed. Recognized Switchify startup entries are migrated to the signed launcher without changing their enabled state; pair Android again after upgrading.

On a fresh unpaired installation, Switchify opens a five-step setup guide once. It checks Bluetooth and input access, links to Switchify on Google Play with a QR code, presents live secure-pairing approvals, and records explicit startup and anonymous-diagnostics choices. **Skip for now** dismisses the automatic prompt without marking setup complete; reopen it at any time from Home or Support. Existing paired users are never forced into the guide.

For UI and hot-reload development only:

```bash
npm run tauri dev
```

`npm run tauri dev` rebuilds an ad-hoc executable whose identity is not stable, so it is unsuitable for macOS Accessibility testing. `npm run dev` starts a browser-only UI shell with sample state. Native Bluetooth, input, startup, tray, secure storage, and updater behavior require a native run command.

## Checks

```bash
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Rust tests use fake input adapters and never control the local pointer or keyboard. Native checks and unsigned bundles run on Windows and macOS in `.github/workflows/ci.yml`.

## Diagnostics

Switchify keeps up to 500 sanitized diagnostic events locally in `diagnostic-history.jsonl`. The history covers application startup, Bluetooth and Accessibility transitions, disconnects, runtime failures, and update checks. It never stores typed text, command payloads, pairing secrets, device names, or full paths; malformed or unwritable history is ignored so diagnostics cannot prevent startup.

Support → Troubleshooting shows a compact summary of recent Bluetooth changes, the last disconnect, and recent errors. Export writes the current sanitized state, the diagnostic schema version, and the complete ordered bounded history to `switchify-diagnostics.json`.

Anonymous diagnostic telemetry is disabled until the user explicitly opts in. Opt-in creates an opaque installation UUID and permits best-effort health reports plus sanitized error reports; retryable error reports are bounded to 20, and opting out deletes the identifier and queue immediately. Builds expose telemetry only when `SWITCHIFY_TELEMETRY_ENDPOINT` is an HTTPS endpoint and `TIMBERLOGS_API_KEY` is supplied from release configuration. Neither value is committed to the repository. See the [privacy policy](https://switchifyapp.com/privacy).

## Windows UIAccess package

Windows grants UIAccess only to a trusted, signed executable installed in a secure location. Sign in to SimplySign Desktop, expose the Certum code-signing certificate, and set its thumbprint before packaging:

```powershell
$env:SWITCHIFY_CERTUM_CERT_THUMBPRINT = '<certificate thumbprint>'
npm run windows:package
npm run windows:verify-package
```

The per-machine NSIS installer places the signed main executable and signed startup launcher under Program Files. Release builds request `highestAvailable` with UIAccess; debug builds remain `asInvoker` without UIAccess. Start with system registers the non-UIAccess launcher, which asks Windows Shell to start the main app hidden.

## Legacy C# release

The C# 0.10.0 application is frozen. Its archival source snapshot is held in the private, read-only `switchifyapp/switchify-pc-legacy` repository for authorized organization members.

Existing public Git history, tags, releases, update metadata, and installer downloads remain in this repository. Installed C# clients continue to use the unchanged public release feed and can update to [v0.10.0](https://github.com/switchifyapp/switchify-pc/releases/tag/v0.10.0). No replacement legacy releases are published.

## Development boundaries

- The macOS development identity is local-only. The application has no production Developer ID signing, notarization, or release publishing workflow; macOS CI builds with `--no-sign`. Windows production packages use the locally available Certum/SimplySign identity and are not published automatically.
- Linux may appear in capability data but is not a supported Bluetooth target.
- Windows Grid 3 output uses the native `Sensory_SwitchInput` broadcast contract. Grid 3 is omitted from macOS capabilities and profiles.
- Update installation requires a signed Tauri update feed. Local development builds can only report updater configuration errors.
