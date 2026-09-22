# Contributor guidance

## Project

Switchify PC is a Rust/Tauri 2 application at the repository root. The React/TypeScript UI lives in `src/`, the Rust backend and platform adapters live in `src-tauri/`, and the vendored CoreBluetooth dependency lives in `vendor/`.

The frozen C# application is not maintained in this repository. Do not reintroduce its source, installer, packaging, or release workflow. Existing public C# tags and releases must remain available unchanged.

## Workflow

- Start every change with a GitHub issue and a scoped branch from current `main`.
- Never create a GitHub issue without a milestone. Select or create the appropriate milestone first and include it in the issue creation request; assigning it afterward is not allowed. This applies to every issue, including bugs, chores, release preparation, and follow-up work. Verify the milestone on the created issue before continuing.
- Choose the milestone from the **next** open release after the current shipped version. Prefer `package.json` / `src-tauri/Cargo.toml` as the shipped version; confirm with the newest published tag including prereleases (`gh release list`), not GitHub's "Latest" release badge (that can point at an older stable while RC tags are current). Do not put new work on a milestone whose version is already shipped, even if that milestone is still open. If the user names a target RC, use that open milestone instead.
- When a version is released, close its milestone if it has no open issues. If open issues remain, move unfinished work to the next open milestone (or ask) before closing the shipped one.
- Close tracking or epic issues when every scoped child issue/PR they list is merged or closed. Do not leave completed tracking issues open with unchecked boxes.
- Keep commits and pull requests focused on that issue.
- Open a draft pull request with the issue-closing reference and validation evidence. Assign the same milestone as the issue.
- Before handoff, deploy an independent agent that did not implement the change to review the pull request's latest head, and address all actionable findings. If review fixes change the head, repeat the independent review on the new latest head.
- Make the pull request ready for review and ensure required CI passes.
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
- Preserve the shipping identity: `Switchify PC`, `com.enaboapps.switchify.pc`, its application data and pairing storage, Accessibility identity, development signing identity, and updater configuration. Identity changes require a separate issue and review.
- Keep native overlays non-focusable, click-through, and synchronized with session cleanup.
- Do not change protocol interfaces or persisted schemas without compatibility tests for existing Android and desktop clients.
