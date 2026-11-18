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

There are no npm exceptions at the moment. GHSA-fc5q-39xc-vjpf (picomatch), excepted
under [#1503](https://github.com/switchifyapp/switchify-pc/issues/1503), was withdrawn by GitHub on 2026-11-18
and removed in [#1524](https://github.com/switchifyapp/switchify-pc/issues/1524).

## Rust

There are no Rust exceptions at the moment.
