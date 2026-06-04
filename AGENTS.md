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

Release automation is not implemented yet. Until a release issue defines the final flow:

- Use semantic versions for early packages.
- Do not tag releases without a dedicated release issue.
- Do not publish installers without a Windows smoke test.
- Do not add code signing, auto-update, WinGet, or cross-platform packaging unless an issue explicitly scopes it.
