# Linux credential worker foundation

Issue #722, dependent on #721. This component is not started by the unavailable Linux runtime. It does not activate Bluetooth, approve pairing, inject input or change existing synchronous model startup. Production ownership and startup migration remain separate work.

One worker owns one dedicated blocking thread and admits at most one outstanding load, save or delete. Overlapping callers receive `Busy`; there is no retry queue or per-request thread spawning. A caller-supplied timeout must be greater than zero and at most 30 seconds. Device identifiers are limited to 128 bytes and tokens to 4096 bytes; empty identifiers/tokens are rejected. Backend errors become the fixed `StorageUnavailable` result, with no raw native error or credential logging.

Timeout and dropping a future invalidate its result. Generation invalidation discards stale results; queued work checks cancellation before calling the backend. Cancellation can race with starting a native call and cannot interrupt it or roll back a save/delete already underway. A timed-out write may still commit. Callers must never publish/approve a credential from a cancelled or failed operation and must reconcile persisted state before retrying a replacement.

Admission remains busy until the native call returns, even after caller timeout. Shutdown stops admission, invalidates results and closes the channel without joining the thread. A stuck native call may therefore leave a detached thread until process exit. The future runtime must own exactly one worker and must not recreate workers on timeout; otherwise repeated recreation would defeat the thread bound. Restart is the recovery path for a permanently stuck worker.

Fake-store tests cover save/load/delete, request and response bounds, sanitized errors, timeout while a call remains blocked, busy admission, discarded stale results, recovery, dropped futures and non-blocking shutdown. They perform no real keyring operations. Physical Secret Service qualification and subscriber isolation are still required before enabling secure Linux pairing; see [credential storage gates](linux-credentials.md).
