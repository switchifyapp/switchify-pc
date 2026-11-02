# Dependency audit exceptions

CI and the release workflow run `npm audit --audit-level=high` and
`cargo audit` over every Rust lock file. The Rust audits deny every advisory
except the ones listed here. Review each exception whenever the dependency it
names changes, and remove it as soon as a fixed release can be used.

There are no exceptions at the moment.
