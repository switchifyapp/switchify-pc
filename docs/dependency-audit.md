# Dependency audit exceptions

CI and the release workflow run `npm audit --audit-level=high` and
`cargo audit` over every Rust lock file. The Rust audits deny every advisory
except the ones listed here. Review each exception whenever the dependency it
names changes, and remove it as soon as a fixed release can be used.

## `zip` 4.6.1 through `tauri-plugin-updater`

`src-tauri/Cargo.lock` ignores RUSTSEC-2026-0367, RUSTSEC-2026-0368 and
RUSTSEC-2026-0369. They concern `ZipArchive::extract()` writing an
attacker-controlled archive to disk: SUID and SGID bits are kept, Unix
permissions are applied through symlinks, and symlink targets may resolve
outside the destination directory. The fix is in `zip` 9. Every release of
`tauri-plugin-updater`, including the newest 2.x and the 3.0 alphas, requires
`zip` 4, so an upgrade alone does not remove the advisories.

The only crate that depends on `zip` is `tauri-plugin-updater`, which compiles
it on Windows alone, to unpack `.zip` update packages. The two advisories about
Unix permission bits therefore never apply to the built binary. Switchify PC
publishes its Windows update as a signed `-setup.exe`, so the `.zip` code path
is never taken either. The plugin's `zip` feature cannot be turned off because
it also enables the `tar` and `flate2` crates the macOS update needs. The
updater installs an update only after verifying its signature with the public
key that `scripts/render-updater-config.mjs` writes into the release
configuration from `TAURI_UPDATER_PUBLIC_KEY`, so an unsigned archive is never
extracted.

Remove these three ignores when `tauri-plugin-updater` moves to `zip` 9.
