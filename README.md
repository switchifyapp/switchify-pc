# Switchify PC

Desktop companion app for Switchify Android.

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

The package script runs `npm run build` first, then creates an unpacked Windows artifact in `dist/win-unpacked` and a per-machine NSIS installer in `dist`.

## Windows uiAccess packaging

Switchify PC uses `uiAccess="true"` so the installed app can interact with elevated or higher-integrity windows for accessibility and input automation scenarios.

Windows only honors `uiAccess` when all of these are true:

- The app executable manifest has `level="highestAvailable"` and `uiAccess="true"`.
- The executable is Authenticode signed.
- The executable is installed in a secure location such as `C:\Program Files\Switchify PC\`.
- Signing happens after icon and manifest resource embedding.

The packaged Windows app also disables the Chromium GPU child-process sandbox at startup. Without that switch, Electron's GPU child process can fail to launch under `uiAccess=true`, causing the app to exit even when the manifest, signature, and install location are valid.

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

Self-signed certificates are for dev/testing only. Production users should not be asked to trust a self-signed certificate manually. Azure Artifact Signing is the preferred low-cost production signing path when eligible; traditional OV/EV code-signing certificates remain possible. Production signing configuration must come from environment variables or CI secrets, never committed files.

## Local network expectations

Switchify PC starts a local WebSocket server on port `7347` by default. Android devices must be on the same local network and able to reach the PC at that port.

Windows Defender Firewall or third-party firewall software may prompt when the packaged app first starts. Allow private-network access for local pairing and control. Public-network access is not required for the MVP.

Do not expose the WebSocket port directly to the internet. Runtime commands are authenticated, but the intended control surface is the trusted local network.

## MVP smoke checklist

Use this checklist after packaging changes and before publishing any installer:

- App launches from `Switchify PC.exe`.
- Tray menu opens and can show the main window.
- WebSocket server starts and shows `Listening on port 7347`.
- Pairing approval requests appear and can be accepted or rejected.
- Android can pair with the PC using local discovery and approval.
- Paired Android device can disconnect and reconnect without deleting the saved pairing.
- Authenticated ping receives an ack.
- Relative mouse movement works and remains responsive under repeated movement.
- Left click works.
- Right-click works.
- Scroll works.
- Text typing works in a focused text field.
- Keyboard shortcut works, for example `Ctrl+C` or `Ctrl+V`.
- Media key command works, for example play/pause or volume up.
- Window control commands work, for example next app and show desktop.
- Disconnect all removes active WebSocket sessions.
- Quit exits the app and removes the tray icon.
