# Contributor guidance

## Project

Switchify PC Preview is a Rust/Tauri 2 application at the repository root. The React/TypeScript UI lives in `src/`, the Rust backend and platform adapters live in `src-tauri/`, and the vendored CoreBluetooth dependency lives in `vendor/`.

The frozen C# application is not maintained in this repository. Do not reintroduce its source, installer, packaging, or release workflow. Existing public C# tags and releases must remain available unchanged.

## Workflow

- Start every change with a GitHub issue and a scoped branch from current `main`.
- Keep commits and pull requests focused on that issue.
- Open a draft pull request with the issue-closing reference and validation evidence.
- Before handoff, make the pull request ready for review, address actionable feedback, reach Greptile 5/5 on the latest head, and ensure required CI passes.
- Do not merge without explicit user instruction.

## Validation

Use Node.js 24 and Rust 1.97.1. Run checks from the repository root:

```bash
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Use fake input adapters in automated tests; tests must never type or move the pointer on the developer's machine.

For macOS Bluetooth and Accessibility testing, use `npm run macos:run`. `npm run tauri dev` is only for UI and hot-reload work because its ad-hoc executable does not have a stable Accessibility identity.

## Architecture and security

- Keep platform-specific behavior behind the existing Rust adapters and preserve equivalent macOS and Windows behavior where supported.
- Preserve framed transport limits, canonical JSON authentication, timestamp and replay checks, constant-time signature comparison, pairing approval, and sanitized state/events.
- Never log or expose received typed text, pairing tokens, authentication signatures, or other secrets.
- Keep input cleanup deterministic across disconnects, authentication shutdown, Bluetooth unsubscribe, and runtime exit.
- Preserve the Preview identity: `Switchify PC Preview`, `com.enaboapps.switchify.pc.preview`, its application data and pairing storage, Accessibility identity, development signing identity, and updater configuration. Promotion to a shipping identity requires a separate issue and review.
- Keep native overlays non-focusable, click-through, and synchronized with session cleanup.
- Do not change protocol interfaces or persisted schemas without compatibility tests for existing Android and desktop clients.
