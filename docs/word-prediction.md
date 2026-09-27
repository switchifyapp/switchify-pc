# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without selecting text or using the clipboard. It deletes nothing, with one bounded exception: when the word has a capital where a lowercase letter was typed, as in “lon” for London or “i” for I’m, it backspaces the typed prefix and types the whole word. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

Predictions use only a temporary buffer of successful Switchify keyboard input, starting with the first letter. Existing text, pasted text and hardware keyboard typing are never read into it. Switchify does not inspect fields, selections, passwords or caret positions. Suggestions may therefore appear anywhere the keyboard is open, including password fields or applications without a text field. The buffer records successful input injection; it cannot verify what an application actually accepted.

The buffer holds at most 512 characters, retaining complete Unicode graphemes at its leading boundary. It tracks ordinary characters, spaces, Backspace and accepted completions across keyboard pages and top/bottom docking. Navigation, Delete, Enter, Tab, shortcuts, failed input, external typing/clicks/scrolling and foreground changes clear context. Opening or closing the keyboard, ending scanning, disconnecting and exiting also discard it, by killing the process that held it. The keyboard remains open across foreground changes, resets modifiers and suggestions, and sends subsequent input to the new foreground application.

A passive observer records only an activity counter and timestamp, never external text. Prediction is unavailable if this observer cannot start or loses access. Edits made before the observer is ready are discarded because intervening activity cannot be verified. Each queued edit is scoped to its foreground target and the time before injection, so edits preceding an observed external change are discarded. Changes within an application that produce no observed input cannot be detected without inspecting its fields.

## On-device model

One Word prediction setting controls an offline SmolLM2-135M int8 ONNX model. The saved `enhancedWordPrediction` field is retained for compatibility but does not choose an engine. The model spells candidates from subword pieces and reads only the last 256 characters of the temporary buffer, beginning at a word boundary. It never learns from typing.

For a first typed prefix, a fixed local context keeps the model from favoring website names at the start of a document; it adds no user text. The model loads in the worker while keyboard input remains available, which takes about a second. Suggestions are blank until loading finishes. Only the first keyboard open of a scanning session waits for it; later opens use the spare worker described under Worker process. A passive badge beside the scan prompt distinguishes loading, a ready keyboard awaiting typed context, no matching suggestions, available suggestions, paused activity tracking, and prediction failure. It never shows typed text and is not a scan target. If loading or inference fails, or a call exceeds 1.5 seconds, the keyboard continues accepting input and offers **Retry predictions** in its toolbar. Retry restarts only the prediction worker, clears its private text context, and leaves the keyboard open; type a new prefix afterward. A 400 ms search budget bounds candidate exploration. A clipped buffer with no complete earlier word yields no suggestions.

Candidates contain ASCII letters and apostrophes and must have sufficient model probability. There is no vocabulary filter: any word the model finds likely can be suggested, including swearing, because the person typing chose it. Single-letter candidates are limited to “a” and “I”. Up to five suggestions are shown: the first, third and fifth are the most likely single words, and the second and fourth are the two most likely two-word phrases that begin with one of the top three words, ranked by the probability of the pair. When fewer phrases are found within an extra 200 ms, single words fill the remaining slots, and the other way round. Accepting a phrase inserts the rest of its first word, a space, the second word and a trailing space.

## Casing

The label on a suggestion always reads as the text will after it is accepted. Casing is decided in three steps.

**1. The word itself.** Ordinary words are lowercase. Names keep the model's capital: mixed case anywhere, such as WhatsApp, or an initial capital mid-sentence that the model clearly prefers, such as London or Monday. The model does that for proper nouns and occasionally a rare word. After a full stop, exclamation or question mark in the text the model read, or when that text is empty, the model capitalises every word, so its capital says nothing about the word and the lowercase form is used; step 3 then decides the first letter. The pronoun I and its contractions, such as I’m and I’ll, always have a capital I.

**2. The letters already typed.** A capital the person typed is kept. A typed prefix of two or more letters, all capitals, is a word being written in capitals, and is completed in capitals whatever the modifiers say. Where the word has a capital and a lowercase letter was typed, accepting restores the capital by retyping the prefix, described below.

**3. The modifiers, for letters not yet typed.** Caps, or Shift locked, uppercases the rest of the word, and together they cancel as they do on typed letters. Before any letter of the word is typed, Shift once changes only the first letter, the way it would change the next typed letter, so under Caps it gives a lowercase first letter. A sentence start capitalises the first letter without any modifier. After typed letters, a pending Shift once is left for the next letter and does not touch the completion.

The keyboard alone decides what a sentence start is: a full stop, exclamation or question mark it typed, not yet followed by a letter and not cancelled by pressing Shift. Punctuation the worker merely sees in its buffer, or the bare start of that buffer after a reset, does not capitalise anything, so a suggestion never disagrees with the keyboard's own automatic Shift. At the true start of a document the keyboard has no such signal, so the first word is not capitalised automatically; Shift once capitalises it.

For the word “water”:

| Typed | Shift | Caps | Sentence start | Suggestion |
|---|---|---|---|---|
| nothing | off | off | no | water |
| nothing | off | off | yes | Water |
| nothing | once | off | either | Water |
| nothing | locked | off | either | WATER |
| nothing | off | on | either | WATER |
| nothing | once | on | either | wATER |
| nothing | locked | on | no | water |
| nothing | locked | on | yes | Water |
| wa | off or once | off | either | water |
| wa | locked | off | either | waTER |
| wa | off or once | on | either | waTER |
| wa | locked | on | either | water |
| Wa | off or once | off | either | Water |
| Wa | locked | off | either | WaTER |
| Wa | off or once | on | either | WaTER |
| Wa | locked | on | either | Water |
| WA | any | any | either | WATER |

### Retyping the prefix

Accepting restores a capital the typed prefix lacks by backspacing that prefix and typing the whole word, as in “lon” for London, “i” for I’m or “wh” for WhatsApp. When the case already matches, which is the usual case, nothing is deleted and only the missing suffix is typed. A capital the person typed is kept, and a word typed in capitals is never retyped.

The deletion is bounded. The prefix is text Switchify typed itself in the current window with no outside activity since. It is never more than one word, and the executor refuses more than 32 deletions. Any keyboard or mouse activity not made by Switchify between choosing the suggestion and typing it cancels the acceptance, so nothing else is deleted.

Two things cannot be guaranteed. Deleting and typing cannot be atomic: if typing fails after the deletion, the prefix is lost and prediction context resets. The deletion also assumes one Backspace removes one typed character; an application that auto-pairs or autocorrects what was typed can break that, and Switchify cannot verify what an application accepted.

The model files are too large to commit. `npm run prediction-model` downloads them from a pinned upstream revision and verifies their SHA-256. The Tauri dev and build commands and CI run it automatically. Run it once before `cargo test` or `cargo clippy` in a fresh checkout.

## Worker process

A separate process owns the buffer, activity observer and model. Private bounded inherited pipes carry successful edits and results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. Timeout stops predictions until the keyboard is reopened; ordinary keyboard operation remains available. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

### Spare worker

Loading the model takes about a second, so a keyboard that had a working worker leaves a spare behind when it closes. The worker that held the typed text is killed and reaped first. A new process is then started, which loads the model and waits. It has received no request, so it holds no text, and it starts its activity observer only on its first request, so it observes nothing while it waits. The next keyboard open adopts it and suggestions are ready without a reload.

At most one spare exists, and only between keyboard opens. It is killed and reaped after two minutes without a keyboard open, when scanning ends, when Word prediction is turned off, on Retry predictions and on exit. A spare started for different switch keys, or one that has exited, is discarded and a new worker is started instead. While it waits, the spare holds the loaded model in memory, roughly 250 MB.

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
6. Close the keyboard, stop scanning, disconnect and exit. Verify input releases and the prediction worker exits. After closing the keyboard one spare worker remains; reopen within two minutes and verify suggestions appear without the loading badge, then verify the spare exits after two minutes idle and when scanning stops. Test observer failure separately; typing should remain usable without predictions.

Compilation and fake-adapter tests do not establish native application compatibility. Record live results separately.
