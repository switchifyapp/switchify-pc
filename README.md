# Switchify PC

Switchify PC is the Rust/Tauri desktop companion for controlling Windows and macOS from the Switchify mobile app. The React/TypeScript interface and Rust backend now live at the repository root. Platform adapters provide Bluetooth LE peripheral support, authenticated pairing, input injection, overlays, profiles, startup and tray behavior, diagnostics, and update checks.

The application uses the shipping product identity `Switchify PC` and bundle identifier `com.enaboapps.switchify.pc`. Only one application may advertise the Switchify Bluetooth service at a time.

## Prerequisites

- Node.js 24
- Rust 1.97.1 through rustup, including `rustfmt` and `clippy`
- Windows: Visual Studio Build Tools with the Desktop development with C++ workload
- macOS: Xcode Command Line Tools and macOS 13.3 or later

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

The signed macOS application stores mobile pairing tokens in `pairing-tokens.json` in its application-data directory. The file is written atomically with user-only `0600` permissions, and its parent directory is restricted to `0700`. Windows uses its native credential store.

The promoted identity starts with new settings, a new desktop ID, and no paired devices. Data, credentials, Accessibility approval, and certificates from earlier development builds are not migrated or removed. Recognized Switchify startup entries are migrated to the signed launcher without changing their enabled state; pair your mobile device again after upgrading.

On a fresh unpaired installation, Switchify opens a five-step setup guide once. It checks Bluetooth and input access, links to Switchify on Google Play with a QR code, presents live secure-pairing approvals, and records explicit startup and anonymous-diagnostics choices. **Skip for now** dismisses the automatic prompt without marking setup complete; reopen it at any time from Home or Support. Existing paired users are never forced into the guide.

Closing the main window keeps Switchify available in the system tray. The tray shows live connection status and provides direct actions for the main window, Settings, Switch Forwarding profiles, disconnecting active devices, and cleanly quitting the background service. Double-clicking the tray icon restores the window on Windows; use **Show Switchify PC** on macOS.

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

## Production releases and signed updates

Production macOS releases are Apple Silicon DMGs signed with an Apple-issued Developer ID Application certificate, submitted to Apple's notarization service, and stapled for offline Gatekeeper verification. A `v*` tag or manual release run creates or updates the matching GitHub Release after verifying the tag, app version, architecture, nested-code signatures, hardened runtime, secure timestamp, and notarization tickets.

The production certificate and App Store Connect API key are held only in the GitHub `production` environment and imported into an ephemeral runner keychain. They are separate from the machine-local `Switchify PC Development` identity used by `npm run macos:run`. See [macOS production releases](docs/macos-releases.md) for certificate creation, GitHub configuration, release, recovery, and rotation instructions.

Packaged release builds check a dedicated Tauri feed at `update-feed/latest.json` shortly after startup, every six hours, and on demand. Settings shows availability, verified download progress, cancellation, retry, installation, and restart state. Concurrent checks, downloads, and installs are deduplicated. Development and unsigned CI builds intentionally keep the updater unconfigured and report that state without crashing.

The `Release Switchify PC` workflow accepts existing `v<version>` tags, renders release-only updater configuration, and publishes signed Apple-silicon macOS and x64 Windows artifacts into one GitHub release. It updates the dedicated feed only after both platform packages pass verification. It does not alter the public C# `v0.10.0` release or its `latest.yml` feed.

Before the workflow can run, configure `TAURI_UPDATER_PUBLIC_KEY`, `APPLE_SIGNING_IDENTITY`, `CERTUM_CERT_THUMBPRINT`, `SWITCHIFY_SUPABASE_URL`, and `SWITCHIFY_SUPABASE_PUBLISHABLE_KEY` as repository variables; configure the Tauri updater private key/password and platform signing credentials as protected secrets. The Windows job targets the self-hosted signing runner with SimplySign available. Private signing material is never generated by or committed to this repository.

## Diagnostics

Switchify keeps up to 500 sanitized diagnostic events locally in `diagnostic-history.jsonl`. The history covers application startup, Bluetooth and Accessibility transitions, disconnects, runtime failures, and update checks. It never stores typed text, command payloads, pairing secrets, device names, or full paths; malformed or unwritable history is ignored so diagnostics cannot prevent startup.

Support → Troubleshooting shows a compact summary of recent Bluetooth changes, the last disconnect, and recent errors. Export writes the current sanitized state, the diagnostic schema version, and the complete ordered bounded history to `switchify-diagnostics.json`.

Protocol v1 clients may attach an optional `deviceName` to an authenticated `connection.ping`. Switchify updates the saved display name for that paired device without changing its device ID, token, or authorization. Empty pings from older clients remain valid, and older PC builds safely ignore the additional payload field. Persistence failures return the sanitized `name_update_failed` response so Remote can retry the name without failing authentication.

Anonymous diagnostic telemetry is disabled until the user explicitly opts in. Opt-in creates an opaque installation UUID and permits best-effort health reports plus sanitized error reports; retryable error reports are bounded to 20, and opting out deletes the identifier and queue immediately. Builds expose telemetry only when `SWITCHIFY_TELEMETRY_ENDPOINT` is an HTTPS endpoint and `TIMBERLOGS_API_KEY` is supplied from release configuration. Neither value is committed to the repository. See the [privacy policy](https://switchifyapp.com/privacy).

## Accounts and settings sync

Signing in under Settings → Account (or the account entry at the bottom of the sidebar) brings a user's settings to their other computers. It uses the same Supabase accounts as Switchify on Android: the user enters their email and the 6-digit code it receives; there is no password. The schema, migrations, and database tests live in the private `switchifyapp/switchify-supabase` repository.

- **What syncs:** pointer, repeat, dwell and cursor settings; custom switch profiles; switch bindings including their keys; point scan and scan preferences; keyboard layout; and remote switch slots. The document is versioned and parsed strictly; a copy written by a newer Switchify PC is never overwritten, and this install asks to be updated instead.
- **What never syncs:** the BLE desktop ID, paired devices and pairing tokens, telemetry consent and install ID, setup progress, and start with system.
- **How:** changes upload a few seconds after they settle, other computers' changes are checked every five minutes and on Sync now, and every write is conditional on the server-owned revision, so concurrent changes merge by section (profiles by ID) instead of overwriting each other; if both computers change the same section (or profile), the account's copy wins, though an edit is never lost to a deletion. The first sync on a computer whose settings differ from the account asks which to keep.
- **Secrets:** requests are made from Rust, so the webview's content security policy stays closed. The refresh token is kept in the OS keychain (service `com.enaboapps.switchify.pc.account`); access tokens stay in memory. Tokens and codes are never logged or sent to the webview. The merge base is kept in `settings-sync.json` in the application configuration folder and contains no secrets.

Accounts are compiled in from two build-time variables, which release builds require:

| Name | Value |
| --- | --- |
| `SWITCHIFY_SUPABASE_URL` | `https://<project-ref>.supabase.co` (must be HTTPS) |
| `SWITCHIFY_SUPABASE_PUBLISHABLE_KEY` | The project's `sb_publishable_…` key |

Both are public values (the Android app embeds them too) and are set as repository variables. Builds without them, including ordinary development builds, show "Accounts are unavailable in this build" and keep the version in the sidebar. To try accounts locally, export both variables before `npm run tauri dev`; note that a dev build signs in against whichever project they name.

Automated tests use fake transports and never touch the network or the keychain. Two ignored tests run end to end against a local stack from `switchify-supabase` (`supabase start`). The account test reads its sign-in code from the stack's Mailpit inbox (`SWITCHIFY_LOCAL_MAILPIT_URL`, default `http://127.0.0.1:54324`); the sync test signs up test users with a password:

```bash
SWITCHIFY_LOCAL_SUPABASE_URL=http://127.0.0.1:54321 \
SWITCHIFY_LOCAL_SUPABASE_KEY=<local publishable key> \
cargo test --manifest-path src-tauri/Cargo.toml local_stack -- --ignored
```

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

- The macOS development identity is local-only. Production Developer ID signing, notarization, Certum/SimplySign signing, and release publication run only through the credential-gated release workflow; ordinary CI remains unsigned.
- Linux may appear in capability data but is not a supported Bluetooth target.
- Windows Grid 3 output uses the native `Sensory_SwitchInput` broadcast contract. Grid 3 is omitted from macOS capabilities and profiles.
- Update installation requires the credential-gated signed Tauri feed. Local development builds can only report updater configuration errors.
