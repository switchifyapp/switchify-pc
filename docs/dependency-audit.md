# Dependency audit exceptions

CI and the release workflow run `node scripts/npm-audit.mjs` and `cargo audit`
over every Rust lock file.

The npm audit fails on every high or critical advisory, dev dependencies
included, except the ones listed, by advisory ID and package, in
`scripts/npm-audit-exceptions.json`. It
also fails when a listed exception no longer matches an advisory, so the
exception is removed as soon as a fixed release is in use. The Rust audits deny
every advisory except the ones listed here.

Review each exception whenever the dependency it names changes, and remove it
as soon as a fixed release can be used. Every exception in
`scripts/npm-audit-exceptions.json` must also be listed below.

## npm

### GHSA-fc5q-39xc-vjpf (picomatch)

[picomatch is vulnerable to ReDoS through single-branch repeated extglobs](https://github.com/advisories/GHSA-fc5q-39xc-vjpf).
Every release up to 4.0.7 is affected, and no patched release exists yet.
Tracked in [#1503](https://github.com/switchifyapp/switchify-pc/issues/1503).

- picomatch is reached only through dev and build tools: vite, vitest,
  tinyglobby and fdir.
- Those tools match only this repository's own glob patterns at build and test
  time. No untrusted input reaches them.
- Nothing from it ships in the app: `npm audit --omit=dev` reports no
  vulnerabilities.

Remove the exception, and update picomatch, once a fixed release is published.

## Rust

There are no Rust exceptions at the moment.
