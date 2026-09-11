# Linux credential storage foundation

Issue #720. This is storage hardening on `linux-support`, not an enabled secure Linux runtime.

The pinned keyring 4.1.6 default `v1` adapter selects Secret Service on Linux, Windows Credential Manager on Windows, and Keychain Services on macOS. Linux continues to use the existing service `com.enaboapps.switchify.pc.pairing` and device-ID lookup keys. No plaintext, kernel-keyring or memory fallback is added. Dependencies, state schema and other platforms' storage selection are unchanged.

The Linux wrapper serializes operations on each storage instance. Save succeeds only when a nonempty token is written and read back unchanged. Missing credentials remain distinct from an inaccessible store; empty stored values fail closed. Backend errors are replaced with a fixed remediation message, never raw D-Bus text, credential attributes or tokens. Unlock the desktop Secret Service keyring; ensure a Secret Service provider is installed/running, then restart Switchify PC. The pinned keyring adapter caches initial store initialization, so merely retrying after an initial service failure may not recover in the same process.

A failed verification does not delete the credential: a write may have succeeded before a read failed, and destructive rollback could erase existing access. A replacement write can therefore change the backend even when verification reports failure; it is not a transaction or a durability guarantee. Future pairing integration must not approve/publish a token on any storage error and must account for replacement failure recovery.

The existing model restores pairing tokens transactionally into the protocol engine. If any load fails, no tokens are activated, and saved pairing metadata is preserved when settings are persisted. Restarting after storage recovery can restore access. Ordinary missing entries retain existing behavior; this wrapper does not infer that every missing entry means the whole store failed.

## Validation and remaining gates

Fake-store tests cover successful write verification, recreated wrapper reads, empty credentials, mismatched read-back, save/load/delete failures, non-destructive verification failure and model metadata preservation/recovery. They never access a real keyring or Bluetooth device. Recreating a fake adapter is not physical persistence evidence.

Before enabling Linux pairing, manually qualify an unlocked store, locked/missing provider, permission denial, application restart, logout/login, failed replacement and explicit forgetting on each supported desktop. No new readiness probe writes test credentials or prompts at startup. Bounded/off-main-thread credential operations and recovery UX belong to production runtime integration; this wrapper retains the synchronous storage API.

Subscriber isolation remains a separate unresolved gate. This change cannot approve Linux pairing or inject input.

Reference: [keyring 4.1.6 v1 adapter](https://docs.rs/keyring/4.1.6/keyring/v1/index.html).
