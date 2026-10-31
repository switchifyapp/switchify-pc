# Offline licence keys

This is the format/generator foundation for #1217. The released app does not
yet accept keys or enforce a trial. This tool is not a payment integration.

Requires Node.js 24; no npm dependencies. Run from the repository root.

## Signing key custody

Choose a key ID such as `pc-2026-1`. Create the directory outside this repository,
with access restricted to the operator. Generate the key pair once:

```sh
node tools/licence-keygen/cli.mjs init --private /secure/licences/signing.pem --public /secure/licences/verify.pem
```

Keep an encrypted offline backup of the signing key. Never put it in Git,
logs, a desktop build, a CSV handover, or a browser. File mode is 0600 on Unix;
on Windows, restrict the directory ACL before running the tool. Files are
created exclusively: existing files and symlinks are never overwritten.

Only the public key and its key ID should be embedded into a signed desktop
release. Retain older public keys when rotating issuers. No production key is
included in this PR. `test-vector.json` uses a published test-only seed.

## Partner batches

```sh
node tools/licence-keygen/cli.mjs batch --private /secure/licences/signing.pem --key-id pc-2026-1 --count 100 --seats 1 --out /secure/licences/partner-batch.csv
```

The output contains one random UUID and signed key per row, with no customer
personal information. Deliver only the CSV, through a private channel. Keep
separate batches for each partner and public checkout; retain an issuance
ledger. The output has mode 0600 on Unix and is never printed to stdout.
An issued key is a bearer credential; protect batches like purchased licences.
Keep the signed key unchanged for resends; do not generate a replacement on
every email retry. This CLI deliberately does not implement a revocation server.

Register each partner credential in the activation ledger before delivery,
using its UUID, SHA-256 digest of the exact UTF-8 key, seats and expiry. The
backend operator RPC is `pc_register_partner_licence`; see switchify-supabase
`docs/licensing.md`. One seat means one active installation. A revoked
computer stops at its next successful check or at its signed allowance expiry.

## Version 1 contract

`SWPC1.<key-id>.<payload-base64url>.<signature-base64url>`

All base64url is unpadded. Ed25519 signs the exact UTF-8 bytes of the first
three components joined by periods, including the product-specific prefix
and key ID. The verifier authenticates these bytes before parsing JSON;
it never reserializes untrusted JSON before verification.

The issuer emits compact JSON fields in this order: `version` (1),
`licence_id` (UUID), `product` (`switchify-pc`), `issued_at` (integer Unix
seconds), `expires_at` (null for perpetual or integer Unix seconds), `seats`
(integer 1–1,000,000). Duplicate/unknown fields are rejected. Timestamps are
nonnegative and at most 253402300799; expiry must follow issuance.
The verifier allows 300 seconds of issue-time clock skew and no expiry grace.
Key IDs contain 1–32 ASCII letters, digits, hyphens, or underscores. The whole
key is limited to 2048 bytes after trimming surrounding whitespace.

Never change this contract without keeping compatibility tests for already
issued keys. Run `node --test scripts/licence-keygen.node-test.mjs` and the
Rust `licence_key` tests to verify the frozen cross-language vector.

## Device allowance contract

`src-tauri/src/licence_lease.rs` verifies the activation service's `SWPL1`
leases and `SWPS1` terminal replies. The app requires a signed allowance for paid access. A purchase key
remains a reusable credential, while paid access requires a device-bound,
time-limited signed allowance.

Lease trust anchors are separate from purchase trust anchors. `LeaseKeys::new`
rejects overlapping issuer IDs and public keys. Prefix and issuer are included
in the signed bytes. Claims bind the product, licence, installation UUID and
public key, generation, server issue time, action and request nonce. Leases
also include a lease UUID and required expiry at most 30 days after issuance.
Cached verification checks the signature, binding and expiry; it needs neither
a network request nor an unlocked keychain. Fresh replies additionally match
the outstanding nonce/action, reject older generations and check freshness.
Only authenticated terminal replies can invalidate a current allowance;
unsigned errors are never revocation evidence.

`activation-test-vector.json` is copied byte-for-byte from the backend fixture
in switchify-supabase PR #19. It contains public test material only. The Rust
tests verify backend-generated lease/status envelopes and reproduce the exact
canonical installation request and signature. Never compile fixture trust
anchors into production builds. Keep this vector synchronized when changing
the activation contract, without changing the frozen SWPC1 purchase format.


Configured builds require `SWITCHIFY_LICENCE_PUBLIC_KEYS` (purchase map),
`SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS` (separate lease map),
`SWITCHIFY_SUPABASE_URL` and `SWITCHIFY_SUPABASE_PUBLISHABLE_KEY`.
Only public material goes in build variables. The release workflow validates
nonempty, separate maps and rejects frozen test signers before building. It
never generates an issuer, deploys activation or bumps the version. Preserve
an encrypted backup of each private signer separately; the desktop never needs
those private keys.

Record v1 migrates to v2 without resetting `first_run` or `last_seen`. A stored
SWPC1 key requires first online activation; it is not a permanent offline
entitlement. The installation private key has a separate OS keyring entry;
record files contain only its public identity and signed allowance. A locked
keychain defers renewal while a valid cached allowance continues to work.
Reinstall with the keychain intact keeps the identity. If the keychain is lost,
explicit transfer recovers the licence using its purchase key. VM clones with
both the credential store and app record copied can share an identity; this
scheme discourages casual sharing, it cannot prevent modified clients or clones.


File persistence and keychain reconciliation run on separate serialized workers,
without holding the entitlement/input mutex during OS I/O. A pending keychain
prompt cannot delay a verified revocation or file persistence. The first mirror read establishes the recovered revision baseline before any
network activation commits; cached access and the UI remain available while
that read waits. Later write completions only acknowledge their snapshot and
cannot replace a newer removal/revocation.
The trial retains its clock high-water mark; paid allowances keep a separate
clock watermark that a fresh nonce-bound signed server reply can recover after
a corrected system-clock jump. Request freshness uses current wall time.
