# Experimental X11 control build

Issue #726; depends on the credential and read-response stack (#721, #723, #725) and Switchify Remote read-v1 support (#157 in switchify-remote). This is a supervised development build, not a Linux production release or a claim of completed hardware qualification.

## Build and opt in

Build as an ordinary user on Ubuntu 24.04/Mint 22 X11, with the development prerequisites from the Linux CI job installed (WebKitGTK 4.1, GTK3, D-Bus, X11/XKB, OpenSSL and AppIndicator headers), Node 24 and Rust 1.97.1:

```sh
npm ci
npm run tauri build -- --debug --no-bundle
SWITCHIFY_LINUX_EXPERIMENTAL=1 SWITCHIFY_LINUX_ADAPTER=hci0 ./src-tauri/target/debug/switchify-pc
```

Without the explicit environment opt-in the existing unavailable Linux runtime remains in use. Requires `XDG_SESSION_TYPE=x11`, a display and no Wayland display environment. The selected adapter must already be powered and advertising-capable. No power, pairing database, D-Bus policy or device permissions are changed. No root UI or input helper is used. Do not use this development mode at the lock screen, unattended, or on a shared desktop; active-seat/lock-screen qualification is not complete.

Use an updated Remote that supports `responseTransport: read-v1`. Compare the pairing verification code in both apps before approving. Approval writes and verifies the credential through Secret Service, persists metadata, then activates and exposes the reply only through the peer-scoped read mailbox. Failures do not approve the device; uncertain replacement writes require restarting and pairing again. Missing/locked Secret Service providers are not bypassed. Startup restoration still uses the existing synchronous model path, so a provider that hangs during startup remains a known limitation.

## Usable slice and limits

Basic text, keyboard shortcuts/modifiers, streamed typing, pointer movement, clicks, drag, scroll and media commands use the existing input adapter. A reduced pointer profile disables repeat, dwell, window management, display navigation and switch forwarding. The Controls UI exposes pointer speed without unsupported repeat/dwell controls. Tray/overlay behavior stays at the existing Linux foundation (visible main window and no overlays). Exact layout, Unicode, scaling and media behavior need manual X11 validation.

One runtime thread owns input and protocol processing. RX admission is bounded to 64 requests of at most 512 bytes. Read replies are bounded by the mailbox. Peer changes, disconnect observations, a two-second missing-poll lease, authentication failure, queue failures, forgetting and exit invalidate stale work and release tracked input. A detected wall-clock pause above three seconds also invalidates the session. Generation-scoped UI approval prevents an old approval from authorizing a replacement connection. Native credential waits are bounded; input is released before waiting. Expired pairing requests are removed.

Radio/daemon failure is fail-closed; automatic service re-registration is not implemented. Restart after such a failure. The app never removes unrelated registrations or pairings. Shutdown requests cleanup and waits up to one second; an irrecoverably blocked native input call is not claimed cancellable. Physical disconnect/address-reuse ordering, cleanup under suspend/lock, Secret Service persistence, and competing ATT clients still require qualification. The read mailbox removes notification broadcasting as the reply mechanism, not all Bluetooth trust concerns.

## Supervised acceptance test

1. Start the opt-in desktop build; confirm the ordinary-user UI reports readiness without granting unavailable capabilities.
2. On the updated phone, discover the PC, compare the verification code and approve on the desktop. Verify pairing completes (not merely that services were discovered).
3. Focus a disposable text editor yourself. Send short test text and a shortcut; test pointer movement, click, scroll, then drag/release.
4. Disconnect while a modifier/drag is held and confirm it is released. Repeat with phone Bluetooth off, app exit and reconnect.
5. Restart the desktop and verify saved pairing reconnects. Repeat with a locked keyring and confirm an actionable failure, never silent approval.

Automated tests use fake input only. No actual typing, clicking, scrolling or pointer movement is part of the test suite. Record exact desktop, BlueZ, adapter, Android and Remote versions with the manual results; never include tokens or typed private text.
