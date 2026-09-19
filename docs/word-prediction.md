# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without deleting text, selecting text or using the clipboard. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

The bundled English SQLite database works offline. Ranking backs off from the longest available three-word context to shorter contexts, then to word frequency. Prefix matching is case-insensitive. Duplicate words are removed. The data is fixed; typing is never used to train or update it.

Windows uses UI Automation and macOS uses Accessibility range attributes. Both read at most 512 preceding characters and a short following range, reject protected fields, and hide predictions for selections or mid-word insertion. Applications that do not expose reliable text ranges may have no predictions. No compatibility claim follows from compilation alone.

If an editable non-protected field can be identified but its text is unavailable, the fallback remembers only successful Switchify typing after a word boundary entered through Switchify. It does not reconstruct existing text. Navigation, deletion, shortcuts, external typing/clicks, focus changes and failures clear this buffer. Fallback is unavailable when its passive activity observer cannot start. The observer records only an activity counter, never external keys or text. Numbers and navigation pages do not retain fallback typing.

A separate process performs accessibility and database work. Private bounded inherited pipes carry results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. Timeout stops predictions until the keyboard is reopened; ordinary keyboard operation remains available. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

Before a suggestion is accepted, the worker checks the field, protection state, text/selection snapshot and external activity again. Any mismatch rejects the action. Verification and native insertion cannot be atomic across applications; a field can still change in that short interval.

## Validation

Automated tests use in-memory databases, fake accessibility adapters and sleeping subprocesses. They never inject desktop input. The database benchmark can be run explicitly with:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml bundled_database_benchmark -- --ignored --nocapture
```

This benchmark prints counts and timings only and does not read a focused field. Its ignored status keeps performance measurement separate from correctness checks.

Native integration checks remain pending on Windows and macOS. The older standalone probe's Notepad result does not qualify this integration. Manually test synthetic text in Notepad, TextEdit, Edge/Chrome, Safari and Word where installed. Exercise typing, prediction acceptance, Shift/Caps, external edits, selection, caret movement, passwords, page changes, top/bottom docking, disconnect and app exit. On macOS use `npm run macos:run` to retain the signed Accessibility identity.

For each app tested, collect at least 100 successful reads and report median, p95, maximum and failures. Target warm context-plus-query p95 below 50 ms; report startup and the 250-ms polling interval separately. Unsupported and untested apps must remain identified as such.

Prepare a disposable field containing `I would like some wa`, with the caret after `wa`. From the repository root run the command below, then focus that field during its five-second countdown. It samples every 250 ms, stops after 100 supported warm samples (or 200 attempts), and prints only counts, fixture comparisons and timings. It sends read-only queries; it cannot accept a word or inject input. The first request includes worker/database startup and is excluded from warm percentiles.

```powershell
node scripts/measure-word-prediction.mjs src-tauri/target/debug/switchify-pc.exe Notepad water
```

For macOS, pass the signed app's `Contents/MacOS/switchify-pc` executable after building with `npm run macos:run`. Use a fresh run for each app. Keep all focused fields synthetic throughout measurement. Repeat functional checks with an empty field, `Hello. wa`, multiline text, `café naïve wa`, a pasted sentence, deletion, a caret inside a word, a nonempty selection, selection replacement, and switching between two fields. Verify that protected fields produce no suggestions and that closing the keyboard removes the worker. Do not count empty/unsupported responses as successful latency samples.

Measured locally on Windows in a debug build (200 synthetic database queries): median 2.59 ms, p95 10.78 ms, maximum 14.35 ms, zero query failures. Database startup took 251.13 ms. These numbers exclude accessibility and IPC and are not app compatibility results.

| App | Integration status |
| --- | --- |
| Notepad, Edge, Chrome, Word on Windows | Untested with this integration |
| TextEdit, Safari, Chrome, Word on macOS | Untested; macOS host unavailable locally |
