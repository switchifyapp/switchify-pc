# Linux BlueZ transport probe

Issue: #716. Stack: #711 → #714 → #715 → this increment.

This is the hardware-test harness for milestone 2 of the [Linux plan](linux-support-plan.md), **not completion of that milestone**. The desktop runtime remains unavailable on Linux. This standalone tool is not packaged, launched by the app, or added to releases. It cannot approve pairing, read stored credentials or inject input. It discards incoming messages and never echoes their contents or identifiers.

## Binding and isolation decision

Use BlueR (`bluer` pinned to 0.17.4, `bluetoothd` feature) for this experiment. It is the BlueZ project's Rust interface and exposes local GATT registration, advertising, request device identity and file-descriptor notifications. A central-only library is unsuitable. Direct D-Bus would still face the same BlueZ delivery constraints while adding object registration and descriptor-lifecycle code to maintain. No new dependency is added to the shipping application; reconsider the binding after the physical tests.

RX uses callback writes (request and command); status uses callback reads. TX uses `AcquireNotify` through BlueR's IO mode so the probe can observe the device address and payload limit associated with a notification writer. It does not use a `StartNotify` broadcast callback for sensitive replies. Notifications contain only a fixed public `linux_transport_probe_only` error envelope, framed by the exact same source as the desktop protocol. The desktop's framing implementation was moved unchanged into `src-tauri/src/ble_wire.rs` and re-exported from `protocol.rs`; existing protocol tests still exercise it.

**Device identity on a writer is not proof of exclusive over-the-air delivery.** BlueZ acknowledges IO subscriptions before the application receives the event. This harness accepts one writer and one RX peer at a time, drops competing writers and never combines fragments from different peers. That is a diagnostic policy, not a qualified security boundary. A refused second subscriber may affect the first session at the daemon level. Do not connect this transport to pairing approval or tokens until the competing-central test proves isolation on supported BlueZ versions. If exclusivity cannot be established, design a separately reviewed enforcement mechanism; do not rely on stopping advertisements or polling connected devices as proof.

The probe stores at most the shared protocol's bounded partial-message set for one peer, plus one active writer. Incoming GATT values are capped at 512 bytes; nonzero write offsets and prepared/reliable writes are rejected. Notifications are generated at a fixed two-second cadence, with a two-second send timeout and no unbounded outbound queue. Only counters, payload limits and fixed diagnostic messages are logged; no payloads, tokens, signatures, device addresses or raw D-Bus errors are printed.

## Build and run

On Linux, install Rust 1.97.1, a C build toolchain, pkg-config, `libdbus-1-dev`, BlueZ and an advertising-capable Bluetooth adapter. The probe does not need GTK/WebKit. Use your ordinary desktop user and the system BlueZ service; do not run it as root or install permissive D-Bus policies to bypass a failure.

```bash
cargo test --locked --manifest-path tools/linux-ble-probe/Cargo.toml
cargo run --locked --manifest-path tools/linux-ble-probe/Cargo.toml -- --help
# Explicitly enables connectable advertising on this already-powered adapter:
cargo run --locked --manifest-path tools/linux-ble-probe/Cargo.toml -- --serve hci0
```

For compilation/testing on hosts without D-Bus headers, add `--features vendored-dbus` before `--`. CI uses the system library. The default invocation and `--help` do not connect to D-Bus or open Bluetooth. Automated tests use in-memory requests only.

Close the desktop companion first to avoid duplicate service UUIDs during the experiment. The probe advertises `Switchify probe`, the existing Switchify service UUID, and the existing RX (write/write-without-response), TX (notify), and status (read) UUIDs. Its status has a fresh `probe-` desktop ID on each run, protocol version 1 and platform `linux`; it does not impersonate a saved pairing identity. It does not change radio power, remove Bluetooth pairings or touch another process's registrations. Ctrl-C or the five-minute deadline drops its registration handles. Crashes also lose the owning D-Bus connection. No automatic daemon/radio recovery is implemented yet; restart the probe after disruptions.

Use the existing Android app to check discovery, status reads, subscription and outgoing framing. Pairing intentionally cannot complete. For writes, small-MTU tests, offset reads and a competing subscriber, also use a GATT test client capable of selecting MTU and write mode. The fixed error response ID is diagnostic, not correlated to the Android request. A UI error alone does not prove notification delivery: record the characteristic traffic using test data only.

Protocol v1's JSON/base64 frame cannot fit the default 20-byte ATT payload. The probe uses BlueR's writer payload limit directly (do not subtract ATT overhead again), refuses an insufficient limit, and asks the operator to test a larger negotiated MTU. This exposes an interoperability constraint rather than inventing a second wire-fragmentation format.

If registration fails, check system D-Bus and bluetoothd availability, ordinary-user permissions, adapter power/rfkill, GATT/peripheral support and available advertising instances. The tool never powers an adapter on or disconnects unrelated peers to make room. A host with a locked-down D-Bus policy remains an unsupported test configuration until its normal-user configuration is understood.

## Cooperative public round-trip verifier

Run the optional `--verify hciN ADDRESS` mode on a **separate Linux adapter/host**, selecting the probe's already-discovered address using normal Bluetooth tooling. Do not use the server adapter as its own central. Stop other clients for this baseline test. The verifier does not scan, change power, pair, trust devices, remove pairings, or send arbitrary data. It refuses an already-connected target and checks the fixed probe status before subscribing or writing. Status checks identify a test fixture, not an authenticated peer.

```bash
cargo run --locked --manifest-path tools/linux-ble-probe/Cargo.toml --features vendored-dbus -- --verify hci1 ADDRESS
```

Replace `ADDRESS` locally with the selected test probe's address; do not paste it into shared diagnostic evidence. Setup is bounded to five seconds, exchange to 25 seconds, and disconnect cleanup to five seconds. Ctrl-C/SIGTERM during exchange also perform bounded cleanup. Cleanup failure is reported explicitly. Avoid another application connecting to the selected test device concurrently: BlueZ connections are shared daemon state, not exclusive client leases.

The verifier reassembles notifications using shared bounded v1 framing and accepts only the exact public probe response. Only then does it write a fixed `linux_transport_probe_receipt` message using request writes and the negotiated payload limit. Confirm both the client's receipt message and an increment of the server's `public_receipts` counter. No received content or IDs are echoed or logged. The server still discards all other messages without execution.

This cooperative test proves delivery in both directions only when physically observed. Its fixed receipt can be forged/replayed, is not authenticated, and is **not** a security or subscriber-isolation mechanism. The periodic response is not correlated to a request. This Linux central test does not establish Android notification receipt. For competing-central qualification, use independent receivers and public test data, observing what each actually receives; a successful single-client test cannot close that gate.

## Observed S26 evidence — 2026-09-11

Server source: `773ffc9094ac302f65c286fdbbc89a661eb7ab3d` (now contained in `linux-support`). Host: Linux Mint 22.3, kernel 7.0.0-31-generic, BlueZ 5.72, Intel USB adapter 8087:0aaa on hci0. Client reported as Galaxy S26; exact Android version and installed version/build were not independently verified. The later supplied log includes the diagnostics introduced in Remote beta.23/build 25.

- First attempt reached MTU negotiation, notification readiness and pairing requested at 12:24:23 UTC. Server opened a notification channel with a 512-byte payload limit and counted two fragments forming one complete message, zero rejections; later observed notification-session cleanup.
- Second attempt resolved the selected PC in about 7.9 seconds (12:27:12–12:27:20 UTC), with status parsing, identity match, MTU and notification readiness all succeeding. Server opened another 512-byte channel; totals reached four fragments and two complete messages, zero rejections. The probe then stopped at its deadline.
- Messages were discarded, not inspected. Their counts do not establish authenticated pairing or execution. Earlier nonmatching candidates and discovery failures did not prevent the later handoff; their root cause remains unproven.
- Phone-side receipt of notifications, second-adapter coverage, isolation, input and credential persistence remain unverified. No public-receipt verifier hardware run has been performed yet.

## Required physical evidence (partially performed)

Record Ubuntu/BlueZ version, kernel, adapter model/firmware, Android model/app version, client tooling and outcome for each test. Use at least two independent adapters and two Android devices. Do not include typed text, secrets or packet dumps containing credentials in evidence.

| Test | Expected observation | Result |
| --- | --- | --- |
| Ordinary user, powered adapter | Advertising and GATT registration; Android sees status and correct UUID/properties | Partial: one adapter/S26 discovered and resolved; see dated evidence above |
| RX request and write-without-response | Complete test messages counted; no execution or echo | Not run |
| Fragmented messages and interleaved IDs | Shared framing limits and expiry enforced | Partial: two complete messages from four fragments; interleaving not physically tested |
| Status offsets and long reads | Reconstructed status matches protocol v1 JSON | Not run |
| Default/small and larger MTUs | Explicit failure at insufficient payload; complete public response at sufficient payload | Not run |
| Subscribe, unsubscribe, reconnect | Channel cleanup and fresh reassembly; no stale fragments | Partial: two notification sessions and one observed session cleanup; stale-fragment hardware test pending |
| Second Android/GATT subscriber | Record whether either peer receives the other's public test stream, and effect of refusing the second writer | Not run; security gate |
| No adapter/daemon, power off, rfkill, advertising exhaustion | Sanitized failure; no radio or unrelated-device changes | Not run |
| Daemon restart, adapter removal, suspend, slow reader | No unbounded queue; restart needed; no persisted state | Not run |
| Ctrl-C, deadline, process termination | Owned service/advertisement disappear; fresh run registers cleanly | Partial: deadline exit observed; complete registration-cleanup matrix pending |

Only successful hardware evidence can close the interoperability/peer-isolation checkpoint. Secure session integration, persistent Linux credentials, reconnect recovery and X11 input remain separate increments.

## Primary references

- [BlueR 0.17.4 local GATT API](https://docs.rs/bluer/0.17.4/bluer/gatt/local/index.html)
- [BlueR notification writer API](https://docs.rs/bluer/0.17.4/bluer/gatt/struct.CharacteristicWriter.html)
- [BlueZ characteristic API: request options and AcquireNotify](https://github.com/bluez/bluez/blob/master/doc/org.bluez.GattCharacteristic.rst)
- [BlueZ advertising registration API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.LEAdvertisingManager.rst)
