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

One seat is a licence condition, not an offline activation counter. A revoked
key cannot be invalidated on a machine that never checks online.

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

## Device allowance contract (activation foundation)

`src-tauri/src/licence_lease.rs` verifies the activation service's `SWPL1`
leases and `SWPS1` terminal replies. This foundation does not yet replace the
app's access policy; the activation integration PR does that. A purchase key
will remain a reusable credential, while paid access requires a device-bound,
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
