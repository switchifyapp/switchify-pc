# Desktop licensing operations

Depends on the version-1 format in `tools/licence-keygen/README.md`.

Set repository variable `SWITCHIFY_LICENCE_PUBLIC_KEYS` before the first paid
release. Its value is a JSON object mapping issuer IDs to raw 32-byte Ed25519
public keys in unpadded base64url. Never put the private key in this variable.
Release jobs pass this public configuration into the compiled Rust verifier.
No variable means licensing is disabled (development/CI and existing releases);
a nonempty invalid configuration locks paid features and reports an error.
Do not publish a paid build without testing its actual public configuration.

Store the matching signing key outside Git and separately in the fulfilment
service's secret store. Generate a partner key and a sandbox purchase key,
then verify both in the actual paid build while disconnected from the network.
Do not embed the test-vector issuer into production.

Local `licence.json` and the separate `com.enaboapps.switchify.pc.licence`
keychain record retain the earliest trial start and latest observed timestamp.
A revision counter ensures a removed key is not restored from an older mirror.
The file is checkpointed about once a minute; the keychain mirror is refreshed
at launch and whenever a key is entered or removed.
Licence state does not enter account settings, pairing records or cloud sync.
The trial starts on the first run of a licensing-enabled build, including
upgrades from older free versions. Offline trial reset prevention is best effort.
A local clock rollback does not rewind the recorded trial timestamp.

Before going live, verify on macOS and Windows: activation, restart offline,
removal, invalid-key replacement, file import, temporary keychain unavailability,
expiry during held input/repeats, and local-switch recovery with focus moving
between applications. Run macOS native checks with `npm run macos:run`.
Automated tests must use fake input and must not read the real clipboard or keychain.
