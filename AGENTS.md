# AGENTS.md

## Project overview

Switchify PC is the desktop companion app for Switchify Android. Its first target is a Windows-first Electron app that accepts authenticated local WebSocket commands from the Android app and turns them into PC mouse, keyboard, text, media, and status actions.

The app should be treated as a trusted local control agent. Be conservative with networking, authentication, logging, and desktop input behavior.

## Work flow

- Every change needs a GitHub issue before implementation.
- Every change needs its own branch from `main`.
- Branch names should be short and scoped, for example `chore/electron-scaffold-1`, `feat/pairing-auth-3`, or `fix/ws-reconnect`.
- Keep pull requests tightly scoped to one issue wherever possible.
- Do not mix implementation work with broad refactors unless the issue explicitly calls for it.
- Prefer existing project patterns once the app scaffold exists.

## Development standards

- Use TypeScript for app code.
- Keep Electron main-process code, renderer code, and shared protocol code clearly separated.
- Keep OS input execution behind a narrow adapter interface so protocol handling is not tied directly to a native automation library.
- Validate all WebSocket messages at runtime before using them.
- Prefer small, testable modules for protocol parsing, pairing, auth, command routing, and desktop input mapping.
- Do not log pairing tokens, shared secrets, raw auth headers, or full typed text payloads.
- Do not add new dependencies without checking whether the existing stack already covers the need.

## Protocol and security

- The PC app must only execute commands from paired Android devices.
- Pairing should use a short-lived code or QR payload and then store a long-lived random shared token.
- Every runtime command must be authenticated before it reaches the desktop input adapter.
- Reject unknown devices, invalid auth, stale timestamps, malformed payloads, and oversized text commands.
- WebSocket errors should return structured error responses and must not crash the app.
- Local-network control is the default assumption. Do not introduce cloud relay behavior unless an issue explicitly requests it.

## Desktop control

- The MVP command set is relative mouse movement, click, right-click, double-click, scroll, key press, shortcut, text typing, media control, and ping.
- Use relative pointer movement first; do not add absolute screen-coordinate mapping unless an issue explicitly requests it.
- Clamp movement, scroll, and text payload sizes to predictable limits.
- Convert native automation failures into structured command errors.
- Manual Windows smoke testing matters for input changes because unit tests should use fake adapters instead of controlling the real OS.

## Electron app guidance

- The main process owns WebSocket server lifecycle, pairing/auth state, tray behavior, and desktop input execution.
- The renderer shows pairing, connection status, server status, and recent non-sensitive errors.
- Communicate between renderer and main process through explicit IPC channels.
- Keep the app useful from the tray: show window, server status, disconnect, and quit.
- Avoid blocking the Electron main process with long-running work.

## Design language

- Use sentence case for user-facing text: headings, navigation, buttons, labels, placeholders, and status.
- Use `...` for in-progress status text until the project standardizes typography.
- Terminal and empty states should end with a period, for example `No devices connected.`
- Keep the UI quiet and utilitarian. This is a background companion utility, not a marketing surface.
- Do not show secrets or token fragments in the UI.

## Testing

Before pushing implementation changes, run the most relevant checks available for the current scaffold. As the repo matures, this should normally include:

```powershell
npm test
npm run build
```

For desktop input or packaging changes, also run the relevant manual smoke path:

- App launches.
- Tray menu works.
- Pairing/status UI opens.
- WebSocket server starts.
- Authenticated ping receives an ack.
- Mouse move, click, right-click, scroll, keyboard shortcut, and text entry work on Windows.

## Release flow

Only the maintainer should publish releases.

Release builds are published from tags named `vX.Y.Z`, where `X.Y.Z` must match `package.json`.

The release workflow is `.github/workflows/release.yml`. It runs on the self-hosted Windows signing runner with the `switchify-signing` label. The runner is expected to have:

- Windows.
- Node and npm.
- GitHub CLI authentication available to the workflow.
- Windows SDK signing tools available, including `signtool`.
- The Certum SimplySign certificate available in `Cert:\CurrentUser\My`.
- An active SimplySign session before the release job runs.

Production release signing uses Certum SimplySign through `signtool`. Do not document or commit the real certificate thumbprint. Use placeholders in docs and examples.

Required signing environment for production packaging:

```powershell
$env:SWITCHIFY_SIGNING_MODE = "certum-simplysign"
$env:SWITCHIFY_CERTUM_CERT_THUMBPRINT = "<certum-certificate-thumbprint>"
$env:SWITCHIFY_CERTUM_TIMESTAMP_URL = "http://time.certum.pl"
```

The release workflow:

- installs dependencies with `npm ci`
- runs `npm run typecheck`
- runs `npm test`
- verifies the Certum signing certificate
- builds native helpers
- packages the Windows x64 NSIS installer with `npm run package:win`
- verifies the tag matches `package.json`
- uploads all top-level `dist` release assets to GitHub Releases, including the signed installer and updater metadata

Local packaging with `npm run package:win` creates artifacts under `dist`. It does not publish a GitHub release.

Before publishing a release, the maintainer should run or confirm the Windows smoke path:

- signed installer verifies with Authenticode
- installer installs per-machine under `C:\Program Files\Switchify PC\`
- `npm run package:win:verify-uiaccess` passes
- app launches and stays running
- tray menu works
- Bluetooth pairing works
- authenticated ping works
- mouse, click, right-click, scroll, keyboard/text, media, and window control commands work
- Settings > Updates can check, download, and show `Install update` in packaged builds
- updater metadata exists in `dist\latest.yml` and packaged `resources\app-update.yml`

To publish a release, the maintainer may push an annotated tag:

```powershell
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

The workflow can also be dispatched manually by the maintainer with a `tag` input.

Do not:

- publish releases from contributor machines
- tag releases without maintainer intent
- hard-code a real certificate thumbprint in docs
- bypass the signed Windows runner for production installers
- publish unsigned production installers
- change signing, updater, WinGet, or cross-platform release behavior unless a dedicated issue explicitly scopes it
