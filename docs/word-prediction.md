# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without deleting text, selecting text or using the clipboard. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

Predictions use only a temporary buffer of successful Switchify keyboard input, starting with the first letter. Existing text, pasted text and hardware keyboard typing are never read into it. Switchify does not inspect fields, selections, passwords or caret positions. Suggestions may therefore appear anywhere the keyboard is open, including password fields or applications without a text field. The buffer records successful input injection; it cannot verify what an application actually accepted.

The buffer holds at most 512 characters, retaining complete Unicode graphemes at its leading boundary. It tracks ordinary characters, spaces, Backspace and accepted completions across keyboard pages and top/bottom docking. Navigation, Delete, Enter, Tab, shortcuts, failed input, external typing/clicks/scrolling and foreground changes clear context. Opening or closing the keyboard, ending scanning, disconnecting and exiting also discard it. The keyboard remains open across foreground changes, resets modifiers and suggestions, and sends subsequent input to the new foreground application.

A passive observer records only an activity counter and timestamp, never external text. Prediction is unavailable if this observer cannot start or loses access. Edits made before the observer is ready are discarded because intervening activity cannot be verified. Each queued edit is scoped to its foreground target and the time before injection, so edits preceding an observed external change are discarded. Changes within an application that produce no observed input cannot be detected without inspecting its fields.

## On-device model

One Word prediction setting controls an offline SmolLM2-135M int8 ONNX model. The saved `enhancedWordPrediction` field is retained for compatibility but does not choose an engine. The model spells candidates from subword pieces and reads only the last 256 characters of the temporary buffer, beginning at a word boundary. It never learns from typing.

For a first typed prefix, a fixed local context keeps the model from favoring website names at the start of a document; it adds no user text. The model loads in the worker while keyboard input remains available. Suggestions are blank until loading finishes. A passive badge beside the scan prompt distinguishes loading, a ready keyboard awaiting typed context, no matching suggestions, available suggestions, paused activity tracking, and prediction failure. It never shows typed text and is not a scan target. If loading or inference fails, or a call exceeds 1.5 seconds, the keyboard continues accepting input and offers **Retry predictions** in its toolbar. Retry restarts only the prediction worker, clears its private text context, and leaves the keyboard open; type a new prefix afterward. A 400 ms search budget bounds candidate exploration. A clipped buffer with no complete earlier word yields no suggestions.

Candidates contain ASCII letters and apostrophes and must have sufficient model probability. There is no vocabulary filter: any word the model finds likely can be suggested, including swearing, because the person typing chose it. Single-letter candidates are limited to “a” and “I”. Up to five suggestions are shown: the first, third and fifth are the most likely single words, and the second and fourth are the two most likely two-word phrases that begin with one of the top three words, ranked by the probability of the pair. When fewer phrases are found within an extra 200 ms, single words fill the remaining slots, and the other way round. Accepting a phrase inserts the rest of its first word, a space, the second word and a trailing space. Ordinary words use lowercase; mixed-case names retain model casing. Casing is applied when the keyboard displays a candidate, and the label shows exactly what will be inserted. Caps, or Shift locked, uppercases the whole completion, and together they cancel as they do on typed letters. Before any letter of the word is typed, Shift once changes only the first letter, the way it would change the next typed letter, and a sentence start capitalises it without any modifier. Modifiers still mirror typing at a sentence start, so Shift once under Caps gives a lowercase first letter. After typed letters, a pending Shift once is left for the next letter and does not touch the completion.

The model files are too large to commit. `npm run prediction-model` downloads them from a pinned upstream revision and verifies their SHA-256. The Tauri dev and build commands and CI run it automatically. Run it once before `cargo test` or `cargo clippy` in a fresh checkout.

## Worker process

A separate process owns the buffer, activity observer and model. Private bounded inherited pipes carry successful edits and results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. Timeout stops predictions until the keyboard is reopened; ordinary keyboard operation remains available. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

Before accepting a suggestion, the worker checks its token, edit revision, foreground identity and external activity. The main process checks its generation and foreground again before injection. Verification and native insertion cannot be atomic across applications; the target can still change in that short interval.

## Validation

Automated tests use fake predictors, foreground/activity sources, input adapters and sleeping subprocesses. They never inject desktop input. They cover first-letter completion, Unicode Backspace, bounded context, stale replies, foreground/external changes, failure cleanup and switch action compatibility. `cargo test` also runs the language model against the fetched files with synthetic text. For release-build suggestions and timings:

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
