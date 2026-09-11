# Linux peer-scoped reply transport

Issue #724. Optional BLE transport extension, retaining protocol v1 messages and authentication. This mailbox is not yet wired to a live runtime.

## Why notifications cannot carry Linux replies

In BlueZ 5.72, `sock_io_read` passes AcquireNotify data to `send_notification_to_devices`, which iterates subscribed device states. Device identity on a BlueR writer is not a delivery boundary. Refusing a competing writer after BlueZ accepts its CCC subscription does not close that race. This is source evidence of broadcast behavior, not merely missing hardware evidence: [pinned BlueZ implementation](https://github.com/bluez/bluez/blob/5.72/src/gatt-database.c#L2445).

## Negotiation and wire contract

A Linux server using this transport adds `responseTransport: "read-v1"` to discovery status and exposes read-only characteristic `7a78f7ec-1d6d-4d92-9ef0-1f89d3db21f4` under the existing Switchify service. Existing service/RX/TX/status identifiers are unchanged. Updated clients choose polling only after reading this marker; absent marker retains existing notification behavior. Unsupported marker values must fail closed. Linux read-v1 servers never place protocol responses on TX, even for older clients. Old clients cannot complete pairing and must update; there is no sensitive notification fallback.

Each offset-zero read consumes one existing v1 JSON/base64 frame (at most 180 bytes); an empty value means no reply. Nonzero offset reads return the same snapshot for ATT long-read assembly. Clients run only one read at a time. A read failure terminates the session rather than retrying a possibly consumed frame. No ACK/retransmission mechanism is added. Clients retain normal bounded protocol reassembly and request deadlines. Reads are directed ATT responses associated by BlueZ with the requesting connection, not notifications.

The runtime must associate RX and mailbox ownership with the BlueZ request peer, reject competing peers, serialize access and clear the mailbox/reassembler/input on disconnect before allowing reuse of an address. Idle mailbox reads never claim ownership. Every async response carries the mailbox generation; results from previous sessions are rejected. Queue admission is atomic and capped at 256 KiB encoded data plus one 180-byte read snapshot. On overflow the runtime must tear down the session, not silently lose a reply.

## Qualification

Automated fake-peer tests cover competing reads without consumption, long-read offsets at MTU 23, normal protocol framing, bounded queues and generation cleanup. No credential is broadcast by this design. Real Android read interoperability, disconnect races and input cleanup still need a supervised test before calling the build usable. Multi-adapter coverage remains a broader release-quality gate, but notification broadcast isolation is no longer the intended trust boundary.
