# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without deleting text, selecting text or using the clipboard. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

Predictions use only a temporary buffer of successful Switchify keyboard input, starting with the first letter. Existing text, pasted text and hardware keyboard typing are never read into it. Switchify does not inspect fields, selections, passwords or caret positions. Suggestions may therefore appear anywhere the keyboard is open, including password fields or applications without a text field. The buffer records successful input injection; it cannot verify what an application actually accepted.

The buffer holds at most 512 characters, retaining complete Unicode graphemes at its leading boundary. It tracks ordinary characters, spaces, Backspace and accepted completions across keyboard pages and top/bottom docking. Navigation, Delete, Enter, Tab, shortcuts, failed input, external typing/clicks/scrolling and foreground changes clear context. Opening or closing the keyboard, ending scanning, disconnecting and exiting also discard it. The keyboard remains open across foreground changes, resets modifiers and suggestions, and sends subsequent input to the new foreground application.

A passive observer records only an activity counter and timestamp, never external text. Prediction is unavailable if this observer cannot start or loses access. Edits made before the observer is ready are discarded because intervening activity cannot be verified. Each queued edit is scoped to its foreground target and the time before injection, so edits preceding an observed external change are discarded. Changes within an application that produce no observed input cannot be detected without inspecting its fields.

The bundled English lookup database works offline. Ranking backs off from the longest available three-word context to shorter contexts, then to word frequency. Prefix matching is case-insensitive. Duplicate words are removed. The data is fixed; typing is never used to train or update it.

## Enhanced word prediction

Enhanced word prediction is off by default. When it is on, a small language model (SmolLM2-135M, int8 ONNX, run with ONNX Runtime on the CPU) spells suggestions from subword pieces, so it can suggest words the lookup lacks, such as WhatsApp. It sees only the buffer described above, including earlier sentences still in the buffer. It runs offline in the prediction worker, never learns, and nothing it reads is logged or stored.

- **Loading and fallback:** the model loads in the background when the worker starts, taking about 0.7 s. Until it is ready, and whenever it fails, the lookup answers as before. Turning the setting on or off restarts the worker.
- **Filters:**
  - words contain only ASCII letters and apostrophes;
  - a word at the start of the text must be capitalised, which rules out subword fragments;
  - single letters must be in the lookup;
  - blocklisted words are never suggested;
  - words the lookup does not know must reach a minimum probability.
- **Spelling:** words the lookup knows take its usual spelling. Unknown words keep the model's spelling, except that a capital coming only from sentence position is removed.
- **Merging:** lookup suggestions fill any remaining positions.
- **Time limits:** the model stops exploring new words after 400 ms. If one call ever takes longer than 1 s, the worker uses the lookup for the rest of its life, so replies stay well inside the two-second deadline.
- **Clipped buffers:** when clipping leaves no complete earlier word, the lookup answers.
- **Cost:** predictions take roughly 40–80 ms on a recent laptop, and the worker needs about 300–450 MB more memory.

The model files are too large to commit. `npm run prediction-model` downloads them from a pinned upstream revision and verifies their SHA-256. The Tauri dev and build commands and CI run it automatically. Run it once before `cargo test` or `cargo clippy` in a fresh checkout.

## Worker process

A separate process owns the buffer, activity observer and database. Private bounded inherited pipes carry successful edits and results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. Timeout stops predictions until the keyboard is reopened; ordinary keyboard operation remains available. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

Before accepting a suggestion, the worker checks its token, edit revision, foreground identity and external activity. The main process checks its generation and foreground again before injection. Verification and native insertion cannot be atomic across applications; the target can still change in that short interval.

## Validation

Automated tests use fixture databases, fake foreground/activity sources, fake input adapters and sleeping subprocesses. They never inject desktop input. They cover first-letter completion, Unicode Backspace, bounded context, stale replies, foreground/external changes, failure cleanup and switch action compatibility. The database benchmark prints counts and timings only:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml bundled_database_benchmark -- --ignored --nocapture
```

`cargo test` also runs the language model against the fetched files with synthetic text. For release-build suggestions and timings:

```powershell
cargo test --release --manifest-path src-tauri/Cargo.toml bundled_model_suggests_current_words -- --nocapture
```

For native validation, use disposable synthetic text in Notepad and a browser on Windows, and TextEdit and a browser on macOS. Launch macOS with `npm run macos:run` to retain its signed Accessibility identity.

1. Open the keyboard from the action menu and an assigned Open keyboard switch, including from idle, a paused scan and an active drag. Verify no click or focus change occurs and owned drag input is released.
2. Type `w`, then `a`, accept `water`, and continue typing. Existing or pasted text must never become prediction context.
3. Change pages and docking, type punctuation and numbers, and return to Letters. Verify context survives these layout changes and Backspace edits the tracked buffer.
4. Use navigation, shortcuts, failed edits and external keyboard/mouse activity. Verify suggestions clear and a new first letter starts fresh context.
5. Change foreground apps with locked modifiers selected. Verify the keyboard stays open, modifiers and suggestions reset, and later keys go to the new app.
6. Close the keyboard, stop scanning, disconnect and exit. Verify input releases and the prediction worker exits. Test observer failure separately; typing should remain usable without predictions.

Compilation and fake-adapter tests do not establish native application compatibility. Record live results separately.
