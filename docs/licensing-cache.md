# Desktop licensing cache

The signed SWPC1 purchase and SWPL1/SWPS1 server formats are unchanged. The local
record is version 3. Versions 1 and 2 remain readable and retain purchase and
trial history, but an unsealed paid cache requires a fresh online activation or
renewal. No previously distributed public build used this paid cache.

Paid records carry an Ed25519 local seal over every record field except the seal
itself, with a cache-specific domain separator. The private installation key is
read from the existing OS credential entry, never from the record. A reply's
public identity or a copied record cannot establish the expected OS identity.
The installation key stays in memory after successful verification for this
process so checkpoints can be authenticated without repeated OS prompts.

Startup publishes a closed paid gate while a worker reads the real installation
key and reconciles the file with the credential-store mirror. Settings and
licence recovery remain available; OS I/O holds no entitlement/input mutex.
An unavailable store retains the record and retries with backoff. Cached paid
access resumes offline only after identity and mirror reconciliation succeed.
A later store outage keeps an already verified allowance intact.

Only authenticated paid records compete by revision. An unsigned/modified file
cannot win using a fabricated revision or bless its imported lease into the
mirror. Trial history is reconciled conservatively. Same-lease checkpoints keep
the greatest authenticated paid-time watermark even if revisions tie; a fresh,
request-bound server proof starts a new clock epoch and can recover from an
incorrect forward clock jump without restarting the trial.

The process clock advances by the greater of wall time and monotonic elapsed
time. Minute checkpoints advance revision and both file/mirror dirty flags and
are resealed. Input gates use the same process clock between UI publications.
Terminal replies also go through a serialized independent mirror writer before
reporting success, so restoring an older file cannot undo a saved revocation.
A stalled writer continues
after the bounded command wait; the current process stays locked and reports
that saving is incomplete. A later server result cannot be overwritten in memory
by a stale mirror completion. Restart must reconcile the mirror before access.

## Threat-model limits

This prevents file-only copying, record edits, replay of an older file against a
surviving newer mirror, and a frozen/backwards wall clock while the app runs.
It does not claim a trusted clock across boots or while the app is stopped.
An administrator who restores the whole OS credential store and files together
and changes the clock can restore an older local state. Sharing a cloned
credential store remains an accepted limitation. No offline client can guarantee
durable knowledge of a revocation if every available persistence operation fails;
the app keeps the current session locked and warns rather than claiming a save.

Tests use public fixture signing keys, fake credential stores and fake transports.
They cover file transplantation, tampering, legacy migration, locked-store cold
restart/healing, same-lease watermark reconciliation, monotonic expiry, signed
clock recovery and revocation recovery after a failed record-file write. Native
OS-store and real switch acceptance remain separate release gates.
