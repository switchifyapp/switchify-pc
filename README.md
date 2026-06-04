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

The package script runs `npm run build` first, then creates an unpacked Windows artifact in `dist/win-unpacked`. This early package is unsigned and does not include auto-update or store publishing.

## Local network expectations

Switchify PC starts a local WebSocket server on port `7347` by default. Android devices must be on the same local network and able to reach the PC at that port.

Windows Defender Firewall or third-party firewall software may prompt when the packaged app first starts. Allow private-network access for local pairing and control. Public-network access is not required for the MVP.

Do not expose the WebSocket port directly to the internet. Runtime commands are authenticated, but the intended control surface is the trusted local network.

## MVP smoke checklist

Use this checklist after packaging changes and before publishing any installer:

- App launches from `Switchify PC.exe`.
- Tray menu opens and can show the main window.
- WebSocket server starts and shows `Listening on port 7347`.
- Pairing code is visible and can be refreshed.
- Android can pair with the PC using the local connection details.
- Paired Android device can disconnect and reconnect without deleting the saved pairing.
- Authenticated ping receives an ack.
- Relative mouse movement works and remains responsive under repeated movement.
- Left click works.
- Right-click works.
- Scroll works.
- Text typing works in a focused text field.
- Keyboard shortcut works, for example `Ctrl+C` or `Ctrl+V`.
- Media key command works, for example play/pause or volume up.
- Disconnect all removes active WebSocket sessions.
- Quit exits the app and removes the tray icon.
