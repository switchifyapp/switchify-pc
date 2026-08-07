# Switchify PC Preview

Switchify PC Preview is the Rust/Tauri desktop companion for controlling Windows and macOS from the Switchify Android app. The React/TypeScript interface and Rust backend now live at the repository root. Platform adapters provide Bluetooth LE peripheral support, authenticated pairing, input injection, overlays, profiles, startup and tray behavior, diagnostics, and update checks.

The Preview keeps its existing product name, bundle identifier, application-data location, pairing storage, Accessibility identity, signing identity, and updater configuration. Only one application may advertise the Switchify Bluetooth service at a time.

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

The command idempotently creates a machine-local, ten-year code-signing identity named `Switchify PC Preview Development`, if needed. Its private key is non-extractable and no certificate, key, or password is stored in Git. Preserve the identity in the login Keychain: deleting or recreating it requires granting Accessibility again.

The first time, choose **Open Accessibility Settings**, enable **Switchify PC Preview**, and return to the app. It silently updates to Ready when the window regains focus. If the row is already enabled but access remains required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry. The setup never resets TCC.

The signed macOS Preview stores Android pairing tokens in `pairing-tokens.json` in its application-data directory. The file is written atomically with user-only `0600` permissions, and its parent directory is restricted to `0700`. Windows uses its native credential store.

For UI and hot-reload development only:

```bash
npm run tauri dev
```

`npm run tauri dev` rebuilds an ad-hoc executable whose identity is not stable, so it is unsuitable for macOS Accessibility testing. `npm run dev` starts a browser-only UI preview with sample state. Native Bluetooth, input, startup, tray, secure storage, and updater behavior require a native run command.

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

## Legacy C# release

The C# 0.10.0 application is frozen. Its archival source snapshot is held in the private, read-only `switchifyapp/switchify-pc-legacy` repository for authorized organization members.

Existing public Git history, tags, releases, update metadata, and installer downloads remain in this repository. Installed C# clients continue to use the unchanged public release feed and can update to [v0.10.0](https://github.com/switchifyapp/switchify-pc/releases/tag/v0.10.0). No replacement legacy releases are published.

## Preview boundaries

- The macOS development identity is local-only. The Preview has no production Developer ID signing, notarization, or release publishing workflow; macOS CI builds with `--no-sign`.
- Linux may appear in capability data but is not a supported Bluetooth target.
- Windows Grid 3 output uses the native `Sensory_SwitchInput` broadcast contract. Grid 3 is omitted from macOS capabilities and profiles.
- Update installation requires a signed Tauri update feed. Local development builds can only report updater configuration errors.
