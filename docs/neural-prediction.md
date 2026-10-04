# Default neural-assisted prediction

Word prediction uses the existing statistical model immediately, then refines its eight-word shortlist with SmolLM2-135M Q8. This runs whenever word prediction is enabled. Existing disabled preferences remain disabled; the compatibility-only enhanced setting remains inert.

Only the isolated prediction worker holds Switchify's tracked typing context. This does not read arbitrary text from focused fields or learn personal text. The neural worker starts on the first prediction, loads once and uses four threads. Windows selects AVX2 only after checking AVX2, FMA and F16C; macOS uses the portable ARM worker. The display keeps its current words selectable while their row is scanned, then applies the queued refinement after leaving the row. Accepting a suggestion uses its exact batch token, never an index into a replacement list.

Neural startup, crashes and the 500 ms inference deadline leave statistical predictions available. There is no automatic neural retry loop; reopening the keyboard or explicitly retrying prediction starts a fresh session. Context invalidation cancels refinement. Windows job containment and macOS process groups cover the prediction process tree. Keyboard closure kills the context-bearing worker; its bounded spare loads only the statistical model until the next prediction.

## Assets and verification

`npm run prediction-model` prepares both sources at build time. It verifies the companion release archive and selected files, fetches pinned upstream sources, runs the converter from the pinned release commit, and checks the converted model hash. Verified inputs are cached in `.cache/prediction-neural`. Corrupt conversion caches fail explicitly. The installed application has no model download path.

Tauri bundles the workers as external binaries so platform signing covers them. The neural resources include the 143,041,952-byte Q8 model, tokenizer, source provenance, Apache model license and worker notices. `scripts/check-packaged-prediction.mjs` checks extracted resources and unsigned worker hashes; signed release checks additionally verify platform signatures. No prediction text or scores are logged.

## Validation and limits

Fake-input tests cover stable scanning, accepting the displayed batch after refinement, token retirement, generation/revision mismatch and missing neural assets. Existing tests cover Unicode/casing, context races, insertion safety, failures and cleanup. CI verifies installer contents and runs 1,000 warmed queries using the production engine and actual neural child, with synthetic input and activity adapters. Timing includes 20 ms refinement polling but excludes the outer desktop pipe and rendering.

Build the integration fixture with `cargo test --release --lib --locked --manifest-path src-tauri/Cargo.toml neural_integration_benchmark --no-run`. Run its reported test executable through `python scripts/measure-neural.py --test-binary TEST_EXECUTABLE --model INSTALLED_ENGLISH_SQLITE --worker INSTALLED_PORTABLE_WORKER --output RESULTS_JSON`. The optional measurement script requires psutil and samples process-tree RSS every 20 ms. No keyboard or pointer input is generated.

Default activation is a product choice, not a new quality qualification. The upstream frozen comparison has two development quality regressions, portable Windows latency misses its target, and synthetic fixtures do not prove unseen-user accuracy. Platform measurements and remaining manual validation are recorded in the PR. Signed macOS manual testing must use `npm run macos:run`; an unsigned CI build cannot establish Accessibility permission behavior.

## Windows reference measurement

The extracted unsigned Windows installer completed 1,000 warmed integration queries with 1,000 neural refinements and no failures. Immediate p95 was 9.23 ms; refinement median was 89.71 ms, p95 91.65 ms and maximum 143.71 ms. Sampled process-tree peak RSS was 693,190,656 bytes, about 661 MiB. The installer was 154,901,216 bytes, about 148 MiB. See `neural-windows-results.json` for machine-readable results and measurement scope.

This uses five repeated synthetic contexts and measures latency, not accuracy. An earlier diagnostic run concurrent with packaging completed 561 refinements before a worker failure left the remaining 439 queries on statistical fallback; the simultaneous build also hit an executable file lock. The final measurement ran after packaging finished and used extracted installer assets. No deadline or runtime retry policy was relaxed.

macOS package and synthetic inference validation run in CI. Signed macOS Accessibility testing and signed Windows installation remain manual release checks; they were not performed on this Windows development host. This PR does not publish a release or change the RC version.
