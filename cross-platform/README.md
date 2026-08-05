# Switchify PC cross-platform preview

This directory contains the parallel Tauri 2 rewrite of Switchify PC. It does not replace or alter the shipping WPF application under `src/`.

The preview uses a React/TypeScript interface and a Rust backend. Platform adapters provide Windows and macOS Bluetooth LE peripheral support, accessibility-aware input injection, persistent authenticated pairing, tray behavior, startup registration, mapped switch sessions, Windows Grid 3 output, diagnostics, profiles, and update checks. The app uses a separate product name, bundle identifier, and data directory so it can be installed beside the shipping application. Only one version may advertise the Switchify Bluetooth service at a time.

## Prerequisites

- Node.js 24
- Rust 1.97.1 through rustup, including `rustfmt` and `clippy`
- Windows: Visual Studio Build Tools with the Desktop development with C++ workload
- macOS: Xcode Command Line Tools and macOS 13 or later

## Run

Install dependencies first:

```powershell
cd cross-platform
npm ci
```

For macOS Bluetooth and Accessibility testing, build and launch the signed debug app:

```bash
npm run macos:run
```

The command idempotently creates a machine-local, ten-year code-signing identity named `Switchify PC Preview Development`, if needed. Its private key is non-extractable and no certificate, key, or password is stored in Git. Preserve the identity in the login Keychain: deleting or recreating it requires granting Accessibility again.

The first time, choose **Open Accessibility Settings**, enable **Switchify PC Preview**, and return to the app. It silently updates to Ready when the window regains focus. If the row is already enabled but access remains required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry. The setup never resets TCC and does not change the separate Switchify Tauri POC identity or permission.

For UI and hot-reload development only:

```powershell
npm run tauri dev
```

`npm run tauri dev` rebuilds an ad-hoc executable whose identity is not stable, so it is unsuitable for macOS Accessibility testing. In a browser, `npm run dev` starts a UI-only preview with local sample state. Native Bluetooth, input, startup, tray, secure storage, and updater behavior require a native run command.

The installed WPF application must be closed before testing preview Bluetooth. Windows input control is available directly; production UIAccess packaging remains owned by the WPF release flow until the preview is promoted.

## Checks

```powershell
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

The Rust tests use fake input adapters and never control the local pointer or keyboard. Native checks run on both Windows and macOS in `.github/workflows/cross-platform.yml`.

## Preview boundaries

- The macOS development identity is local-only. The preview has no production Developer ID signing, notarization, or release publishing workflow; macOS CI explicitly builds with `--no-sign`.
- Linux is represented in capability data but is not a supported Bluetooth target.
- Windows Grid 3 output is available through the native `Sensory_SwitchInput` broadcast contract. Grid 3 is omitted from macOS capabilities and profiles.
- UIAccess packaging, mouse repeat, and display navigation are not advertised by the preview.
- Update installation requires a signed Tauri update feed; local development builds can only report updater configuration errors.
