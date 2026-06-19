# Switchify PC

Switchify PC is the Windows desktop companion for Switchify Android. It runs in the tray, accepts authenticated Bluetooth commands from paired Android devices, and turns them into mouse, keyboard, text, media, window, and status actions on the PC.

Switchify PC is early-stage Windows-first software. Expect Bluetooth and Windows packaging behavior to be the main supported path for now.

## Download

Download the latest Windows installer from GitHub Releases:

- [Latest Switchify PC release](https://github.com/switchifyapp/switchify-pc/releases/latest)

Install Switchify Android from Google Play:

- [Switchify for Android](https://play.google.com/store/apps/details?id=com.enaboapps.switchify)

The Windows installer is a per-machine installer and should be installed under `C:\Program Files\Switchify PC\` so Windows can honor `uiAccess`.

## Requirements

- Windows 10 or later.
- Bluetooth enabled on the PC.
- Switchify Android installed on a nearby Android device.
- Per-machine install under `C:\Program Files\Switchify PC\` for full `uiAccess` behavior.

## Using Switchify PC

1. Install Switchify PC.
2. Install Switchify Android.
3. Launch Switchify PC and leave it running in the tray.
4. Open Switchify Android near the PC.
5. Approve the pairing request on the PC and confirm the verification code.
6. Use the Android app to control the PC.

If the main window is closed, Switchify PC continues running from the tray. Use the tray menu to reopen it or quit.

## Development

Install dependencies:

```powershell
npm install
```

Run the Electron app in development mode:

```powershell
npm run dev
```

Build the compiled Electron output:

```powershell
npm run build
```

Build the native Windows cursor overlay helper:

```powershell
npm run native:build-overlay
```

Run checks before pushing implementation changes:

```powershell
npm run typecheck
npm test
```

## Windows packaging

Create a local Windows x64 package:

```powershell
npm run package:win
```

The package script runs `npm run build` and `npm run native:build-overlay` first, then creates an unpacked Windows artifact in `dist/win-unpacked` and a per-machine NSIS installer in `dist`.

Local packaging builds artifacts under `dist`. It does not publish a GitHub release.

## Windows uiAccess packaging

Switchify PC uses `uiAccess="true"` so the installed app can interact with elevated or higher-integrity windows for accessibility and input automation scenarios.

Windows only honors `uiAccess` when all of these are true:

- The app executable manifest has `level="asInvoker"` and `uiAccess="true"`.
- The executable is Authenticode signed.
- The executable is installed in a secure location such as `C:\Program Files\Switchify PC\`.
- Signing happens after icon and manifest resource embedding.

The packaged Windows app also disables the Chromium GPU child-process sandbox at startup. Without that switch, Electron's GPU child process can fail to launch under `uiAccess=true`, causing the app to exit even when the manifest, signature, and install location are valid.

Packaged Windows builds include `SwitchifyCursorOverlay.exe`, a native self-contained cursor overlay helper under app resources. The Electron app controls it over stdin JSON lines so cursor feedback can render as a per-pixel-alpha ring over protected UI. If the helper cannot start, Switchify PC falls back to the Electron overlay instead of failing input commands.

Development builds can use a local certificate chain trusted on the test machine. The script creates a local dev root CA, creates a code-signing leaf certificate, exports the leaf PFX, stores the leaf thumbprint for `signtool`, and imports trust material into the current user and local machine certificate stores. Accept the UAC prompt so Windows can trust the certificate machine-wide for `uiAccess`.

```powershell
$env:SWITCHIFY_DEV_CERT_PASSWORD = "choose-a-local-password"
npm run signing:create-dev-cert
```

Then package and verify:

```powershell
$env:SWITCHIFY_DEV_CERT_PASSWORD = "same-password"
npm run package:win
npm run package:win:verify-uiaccess
```

Run the generated installer from `dist` and install per-machine. To verify the installed Program Files copy launches and stays running, use:

```powershell
$env:SWITCHIFY_VERIFY_INSTALLED_LAUNCH = "1"
npm run package:win:verify-uiaccess
```

Running from `npm run dev`, `dist/win-unpacked`, AppData, Downloads, or the repo does not prove that `uiAccess` is active.

Self-signed certificates are for development and testing only. Production users should not be asked to trust a self-signed certificate manually.

## Production signing

Production Windows packages are signed with the Certum SimplySign code-signing certificate through `signtool`.

Required environment variables:

```powershell
$env:SWITCHIFY_SIGNING_MODE = "certum-simplysign"
$env:SWITCHIFY_CERTUM_CERT_THUMBPRINT = "<certum-certificate-thumbprint>"
$env:SWITCHIFY_CERTUM_TIMESTAMP_URL = "http://time.certum.pl"
```

The release workflow expects the Certum certificate to be available in `Cert:\CurrentUser\My` on the Windows signing runner and the SimplySign session to be available before the release job runs.

## Release CI

Only the maintainer should publish releases.

Release builds are published from tags named `vX.Y.Z`, where `X.Y.Z` matches `package.json`.

The release workflow runs on the self-hosted Windows signing runner with the `switchify-signing` label. It:

- installs dependencies with `npm ci`
- runs `npm run typecheck`
- runs `npm test`
- verifies the Certum signing certificate
- builds native helpers
- packages the Windows x64 NSIS installer
- verifies the tag matches `package.json`
- uploads the installer and update metadata to GitHub Releases

The maintainer can publish a release by pushing an annotated tag:

```powershell
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

The workflow can also be dispatched manually by the maintainer with a `tag` input.

## Bluetooth connection expectations

Switchify PC uses Bluetooth for PC control pairing and reconnect. The Android device must be near the PC, Bluetooth must be enabled on both devices, and the first pairing still requires approval on the PC.

Paired devices reconnect over Bluetooth using the existing app-level pairing token and authenticated command flow. Local-network WebSocket control, mDNS discovery, manual IP entry, and QR connection are not part of the product path.

## Security

Bluetooth proximity is not authentication. Pairing approval and authenticated commands remain required, and pairing tokens, auth proofs, and typed text payloads must not be exposed in logs or UI.

Please report vulnerabilities by email to owen@switchifyapp.com instead of opening public issues.

## MVP smoke checklist

Use this checklist after packaging changes and before publishing any installer:

- Signed installer verifies with Authenticode.
- Installer installs under `C:\Program Files\Switchify PC\`.
- App launches from `Switchify PC.exe`.
- Tray menu opens and can show the main window.
- Main window shows Android download QR/link before a device is connected.
- Android download link opens externally in the browser.
- Bluetooth helper starts and reports a safe status.
- No QR/manual local-network connection UI appears for pairing or control.
- No local IP address or WebSocket address appears in Settings or troubleshooting.
- Pairing approval requests appear and can be accepted or rejected.
- Android can pair with the PC using Bluetooth and approval.
- Paired Android device can disconnect and reconnect without deleting the saved pairing.
- Authenticated ping receives an ack.
- Relative mouse movement works and remains responsive under repeated movement.
- Cursor highlight appears as a green ring, not a square, including over the Start menu.
- Cursor highlight does not steal focus or block clicks.
- Left click works.
- Right-click works.
- Scroll works.
- Text typing works in a focused text field.
- Keyboard shortcut works, for example `Ctrl+C` or `Ctrl+V`.
- Media key command works, for example play/pause or volume up.
- Window control commands work, for example next app and show desktop.
- Settings > Updates can check for updates in packaged builds.
- Downloaded updates show an `Install update` action.
- Disconnect all removes active Bluetooth sessions.
- Quit exits the app, removes the tray icon, and exits the native cursor overlay helper.

## License

Switchify PC is licensed under the GNU Affero General Public License v3.0 or later. See `LICENSE`.
