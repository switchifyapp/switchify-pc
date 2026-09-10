# Linux implementation plan

Tracking issue: [#710](https://github.com/switchifyapp/switchify-pc/issues/710) (planning only).
Source baseline: `d222639`, version `1.0.0-rc.6`.
Status: proposed implementation sequence; no Linux runtime support is delivered by this document.

## Goal and support boundaries

Enable the existing Switchify Android client to securely pair with and control a Linux desktop. Preserve the product identity, protocol v1, pairing approval, persisted settings, and existing macOS and Windows behavior.

The proposed first supported configuration is Ubuntu 24.04 x86_64 with an X11 desktop session, BlueZ, a Bluetooth adapter that successfully supports connectable LE advertising, and an available persistent desktop credential store. Treat Debian as a subsequent validation target until an exact release and desktop have passed the matrix below. A successful Linux build alone does not qualify a platform as supported.

The first public milestone includes pairing, reconnection, text and keyboard input, pointer movement, clicks, dragging, scrolling, switch profiles, repeat controls, diagnostics, startup, and recoverable background operation. Cursor feedback and dwell must either pass their acceptance tests or be explicitly disabled in UI and wire capabilities. Grid 3 and Windows UIAccess stay unavailable. Window management commands and display navigation are exposed only after their desktop-specific behavior is validated.

Native Wayland support is a separate milestone. A working XWayland window does not establish control of native Wayland applications. Do not label the first milestone as general Linux support without stating the X11 requirement. ARM64, Flatpak, Snap, RPM, and broad distro coverage follow independently.

## Existing foundation and gaps

| Area | Current source | Implementation implication |
| --- | --- | --- |
| Runtime dispatch | [lib.rs](../src-tauri/src/lib.rs), [macos.rs](../src-tauri/src/macos.rs), [windows_runtime.rs](../src-tauri/src/windows_runtime.rs) | Add Linux implementations for installation, shutdown, access checks, pairing approval/rejection, disconnect, repeats, and dwell. Runtime hooks currently exist only for macOS and Windows. |
| BLE and protocol | [protocol.rs](../src-tauri/src/protocol.rs), [ble_lifecycle.rs](../src-tauri/src/ble_lifecycle.rs) | Reuse authentication, framing and recovery machinery. Add a BlueZ server transport; selectively expose reusable items currently gated to macOS/tests. |
| Input | [input.rs](../src-tauri/src/input.rs), [Cargo.toml](../src-tauri/Cargo.toml) | `InputInjector` and `DesktopInput` provide a seam for Linux. Enigo 0.6.1 is pinned; generic injection exists, but Linux window actions return an error. |
| Visual feedback | [overlay.rs](../src-tauri/src/overlay.rs), [modifier_overlay.rs](../src-tauri/src/modifier_overlay.rs), [display_navigation.rs](../src-tauri/src/display_navigation.rs) | Cursor overlay lacks a Linux module. Modifier overlay has a generic webview path requiring runtime verification. Display calculations can be reused after coordinate validation. |
| Capabilities and UI | [state.rs](../src-tauri/src/state.rs), [App.tsx](../src/App.tsx), [api.ts](../src/api.ts) | Linux exists as a capability value with overlays/navigation disabled. Replace binary platform assumptions, setup text, sample state and unconditional feature claims. |
| Persistence/startup | [storage.rs](../src-tauri/src/storage.rs), [lib.rs](../src-tauri/src/lib.rs) | Non-macOS token storage uses keyring; verify the actual pinned Linux backend and persistence. Autostart has a generic non-Windows path. |
| CI and release | [ci.yml](../.github/workflows/ci.yml), [release-tauri.yml](../.github/workflows/release-tauri.yml), [create-update-feed.mjs](../scripts/create-update-feed.mjs) | Native CI covers Windows/macOS only. Release artifact selection and updater keys assume those two platforms. |

Static inspection identifies missing runtime and overlay symbols on Linux. The initial assessment did not execute a native build because Cargo was unavailable in that environment; milestone 1 must establish the complete compiler/dependency baseline.

## Ordered implementation milestones

Each milestone becomes a separate implementation issue and focused branch from current `main`, with a draft PR, validation evidence, and independent review under [AGENTS.md](../AGENTS.md). Closing #710 accepts the plan only. Do not treat it as completion of Linux support. Keep Linux disabled for public releases until the release gates pass.

### 1. Establish a Linux build and test baseline

- Install Node.js 24 and Rust 1.97.1 and resolve the Linux development libraries required by the pinned Tauri, Enigo and credential dependencies, including WebKitGTK 4.1 and AppIndicator integration.
- Add `linux_runtime.rs` and a Linux cursor-overlay module or explicit unavailable implementation behind `cfg(target_os = "linux")`. Make unsupported features safe at startup and correctly represented in capabilities; do not grant input access through a stub.
- Audit all non-Windows/non-macOS branches, including shutdown, modifier overlay initialization, tray availability, and unsupported window actions. Optional overlay or tray failure must not abort the usable main window.
- Add Ubuntu native build, Clippy and tests to CI. Use injected fake adapters; a headless test job must not control the runner's input or need physical Bluetooth hardware.
- Extract shared runtime command/session logic only where needed for the Linux backend; test existing platform behavior before moving it.

Exit: Linux compiles and launches its UI with honest unavailable states; existing native jobs remain green. Record exact build/runtime packages and the tested desktop session.

### 2. Prove BlueZ peripheral interoperability

- Evaluate a maintained Rust BlueZ server binding versus direct D-Bus integration against the required APIs before selecting a dependency. Central-only scanning libraries are insufficient.
- Implement local GATT service registration and connectable advertising using the existing service, RX, TX and status UUIDs and characteristic properties.
- Prototype reads, writes, offsets, write-without-response, notification subscription, fragmentation and negotiated MTU behavior using the existing Android app and at least two BLE adapters.
- Establish how incoming requests identify a peer and how outbound replies are delivered. BlueZ subscription notifications may not provide the same per-central targeting as other backends: prove that a pairing token or response cannot be delivered to an unintended subscriber. If targeted delivery cannot be guaranteed, enforce a tested single-peer policy before returning sensitive data.
- Record ordinary-user D-Bus permissions, missing adapter/daemon behavior, advertising limits, rfkill and powered-off behavior. Do not run the UI as root or assume every Bluetooth adapter can advertise.

Exit: Android discovers the desktop and exchanges the existing framed protocol through BlueZ. Document peer/subscription isolation, adapter requirements and the chosen transport library. This is the first technical go/no-go checkpoint.

### 3. Integrate secure sessions, storage and recovery

- Route the BlueZ transport through the existing protocol engine. Preserve canonical JSON authentication, frame/queue limits, timestamps, replay protection, constant-time comparison, pairing approval, and sanitized errors.
- Serialize transport events and input operations with an explicit runtime owner. Invalidate stale callbacks and queued notifications using session/recovery generations. Avoid holding the shared model lock over D-Bus operations.
- Implement bounded notification backpressure and error handling. Reuse recovery scheduling for radio changes, adapter removal, daemon restart and suspend/resume; prevent duplicate service registrations or advertisements.
- Make disconnect, unsubscribe, authentication failure, timeout, device forgetting, suspend and application exit cancel repeats/dwell and release held keys/buttons. Test cleanup after partial injection failures.
- Verify a persistent Linux credential backend in the pinned keyring version. Handle absent/locked stores with actionable state, preserve saved pairing records on storage failure, and avoid silently accepting a pairing that cannot persist. Test restart and logout/login; do not silently fall back to plaintext or volatile storage.
- Retain single-instance behavior and diagnose conflicts without removing another process's advertisements or unrelated Bluetooth pairings.

Exit: secure pairing survives restart; all teardown paths clean up deterministically; repeated power/daemon disruptions recover without stale commands. Authentication and storage regression tests pass across platforms.

### 4. Deliver and validate X11 input

- Instantiate Enigo through the existing input abstraction and report access according to usable session/backend state. Detect a Wayland session explicitly and show the X11 requirement until milestone 7 passes.
- Validate text, shortcuts, modifier latching, switch press/release, pointer scale, dragging, both scroll axes, media controls, mouse/key repeat and disconnect cleanup. Cover Unicode, non-US layouts, physical modifier interaction and repeated identical keys.
- Implement Linux window commands only for documented desktop mappings, or expose unsupported results consistently. Audit Android supported-command lists as well as desktop controls. Add backward-compatible capability tests for any protocol additions; inspect Android consumption before promising that no client change is necessary.
- Wire repeat cancellation and dwell through real Linux runtime hooks. Keep unsupported controls disabled rather than accepting operations with no effect.

Exit: existing Android control works in real X11 applications under a normal user, with no stuck input after forced disconnect. Every advertised command has automated adapter coverage and a recorded manual result.

### 5. Add feedback and desktop integration

- Reuse shared cursor rendering for a Linux overlay. Verify transparency, click-through, non-focusability, stacking, fullscreen/workspace behavior and deterministic hiding on session end.
- Test the generic modifier overlay and disable it gracefully if unsupported. Never allow a feedback window to steal keyboard focus.
- Verify pointer position and monitor coordinates, including negative origins, scaling, hotplug and display navigation. Enable capabilities only after validation. Dwell must have reliable pointer tracking and visible progress/cancellation feedback before being enabled.
- Validate tray menu access, close-to-background and start-hidden behavior on the selected desktop. Provide an accessible recovery path when no tray host is present so closing the window does not strand the user.
- Test startup registration with installed paths, moved AppImages, duplicate instances and disabled startup. Update setup/support text with Bluetooth, credential-store and session-specific remediation.

Exit: supported desktop integration works without focus theft; missing optional components leave a usable app; capabilities match measured support.

### 6. Package and release the X11 milestone

- Produce an x86_64 `.deb` and AppImage with the existing identity and Linux-specific dependency metadata. Build on the oldest selected supported base with compatible WebKitGTK/glibc; test installation on clean machines.
- Use signed Tauri updater artifacts for the AppImage path after validating the pinned updater plugin's Linux format and restart behavior. `.deb` installations should use package-managed/manual package upgrades and must not attempt to replace themselves with an AppImage.
- Extend artifact discovery, signature/format verification, feed tests and release workflow with `linux-x86_64` only when the Linux release gate is enabled. Preserve existing Windows/macOS keys and legacy release metadata.
- Publish the combined feed only after every platform required for that release passes verification. Before Linux is enabled, Linux experimentation must not block existing releases.
- Test fresh install, pairing persistence through upgrade, cancelled download, invalid signature, interrupted update, unwritable installation path, clean restart and recovery using a previously published package. Uninstall must not unexpectedly erase pairing/settings data.
- Document exact supported OS, desktop/session, architecture and Bluetooth prerequisites. Keep ARM and additional formats out of the first release claim.

Exit: install and upgrade succeed on clean supported machines, signed updater checks pass, and the complete X11 acceptance matrix has evidence attached to the release issue.

### 7. Qualify Wayland as a separate support milestone

- Prototype consent-based RemoteDesktop portal/libei input for GNOME and KDE, recording exact compositor and portal versions. Evaluate text/layout fidelity, relative/absolute movement, coordinate mapping, session restoration, permission denial and logout/suspend behavior.
- Evaluate compositor-specific protocols only as explicitly scoped alternatives. Enigo's experimental backends are feasibility starting points, not a support guarantee.
- If a kernel `uinput` helper is necessary, write a separate architecture/security decision covering authenticated local IPC, minimum permissions, active-seat/session restrictions, lock-screen behavior, bounded command validation, cleanup, helper updates and uninstall. Avoid world-writable device access and a root UI. A virtual input device does not by itself solve global pointer observation, monitor navigation or overlays.
- Independently qualify pointer observation and overlay positioning. Keep unavailable feedback/navigation features disabled on compositors that cannot provide them; require suitable feedback before enabling dwell.
- Test native Wayland and XWayland applications, denied/revoked permission, startup without a restored session, remote/local session boundaries, and lock/logout transitions. Publish a compositor-specific feature matrix.

Exit: choose and document the supported backend and permission UX, then enable only the compositor/features proven by the matrix. If no acceptable route passes, retain the explicit X11 support boundary and record the unresolved requirements.

## Validation matrix and release evidence

| Dimension | Required evidence |
| --- | --- |
| Transport hardware | At least two independent advertising-capable adapters and two Android devices; discovery, pairing, small MTU, subscription churn, queue saturation, disconnect and reconnect. |
| Trust boundaries | Unapproved/invalid/replayed commands rejected; competing subscribers cannot receive secrets; no typed text, tokens, signatures or raw payloads in diagnostics. |
| Lifecycle | Bluetooth off/on, rfkill, adapter removal, daemon restart, suspend/resume, app exit/crash and device forgetting; verify no stuck keys/buttons or replayed queued input. |
| Input | X11 real applications, Unicode and multiple keyboard layouts, modifiers, media, scrolling, dragging and repeat controls; fake injectors for automated coverage. |
| Desktop | Selected X11 desktop with/without tray host, multiple monitors/scales, negative monitor coordinates, fullscreen, workspace changes and autostart. |
| Persistence | Available, absent and locked credential store; failed writes; restart/logout; update preserves identity, settings and pairings. |
| Distribution | Clean Ubuntu 24.04 x86_64 install, `.deb` upgrade, signed AppImage update/restart, failure recovery. Additional releases/desktops require their own recorded results. |
| Existing clients | Protocol fixtures and current Android interoperability; unchanged behavior for Windows/macOS; older capability consumers handle additions safely. |
| Wayland (later) | Exact GNOME/KDE and portal versions, native apps, consent denial/revocation, session restoration, lock/logout and per-feature limitations. |

Run the repository-required frontend lint/tests/build and Rust format/Clippy/tests with the pinned toolchains for implementation PRs, plus Linux native bundling. Use mocked D-Bus, credentials and input for CI, and an explicitly operated physical test system for hardware checks. Repeat existing Windows and macOS validation for shared-code changes. Independent review must cover the latest PR head; re-review after fixes. Do not merge without user instruction.

## Sequencing, estimates and decisions

Milestone 1 precedes 2; 2 precedes secure transport integration in 3; 3 and 4 must both pass before end-to-end qualification in 5 and release work in 6. The Wayland feasibility investigation can begin after the input seam in 4 exists, but it does not gate the explicitly X11 release.

Planning estimates for one experienced Rust/Linux engineer are 1–2 weeks for a hardware prototype, 4–6 weeks total for a constrained X11 milestone, and 8–12+ weeks total for production qualification including selected Wayland desktops. These are provisional ranges, not commitments. Re-estimate after the BlueZ peer-isolation prototype and Wayland input/overlay tests; hardware access and upstream limitations can dominate elapsed time.

Before expanding release scope, record decisions on the exact desktop matrix, Bluetooth adapter requirements, credential backend, per-peer notification safety, update ownership by package format, and Wayland permissions. Create implementation issues as work is scheduled rather than assuming all later milestones are already approved for release.

## Technical references

Verify these APIs against the chosen dependency and OS versions during implementation:

- [BlueZ local GATT registration](https://github.com/bluez/bluez/blob/master/doc/org.bluez.GattManager.rst)
- [BlueZ advertising API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.LEAdvertisement.rst)
- [Enigo 0.6.1 Linux backends and limitations](https://docs.rs/crate/enigo/0.6.1)
- [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri AppImage distribution constraints](https://v2.tauri.app/distribute/appimage/)
