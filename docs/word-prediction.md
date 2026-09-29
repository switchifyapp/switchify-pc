# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without selecting text or using the clipboard. It deletes nothing, with one bounded exception: when the word has a capital where a lowercase letter was typed, as in “i” for I’m, it backspaces the typed prefix and types the whole word. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

Predictions use only a temporary buffer of successful Switchify keyboard input, starting with the first letter. Existing text, pasted text and hardware keyboard typing are never read into it. Switchify does not inspect fields, selections, passwords or caret positions. Suggestions may therefore appear anywhere the keyboard is open, including password fields or applications without a text field. The buffer records successful input injection; it cannot verify what an application actually accepted.

The buffer holds at most 512 characters, retaining complete Unicode graphemes at its leading boundary. It tracks ordinary characters, spaces, Backspace and accepted completions across keyboard pages and top/bottom docking. Navigation, Delete, Enter, Tab, shortcuts, failed input, external typing/clicks/scrolling and foreground changes clear context. Opening or closing the keyboard, ending scanning, disconnecting and exiting also discard it, by killing the process that held it. The keyboard remains open across foreground changes, resets modifiers and suggestions, and sends subsequent input to the new foreground application.

When context is cleared inside a word and the caret has not moved away, the letters that finish that word would be completed as if they began a new one: after “hel”, a further “l” could be completed to “like” and leave “hellike”. Suggestions are therefore held until a space, punctuation or other character that ends the word has been typed, and resume with the next word. The badge reads “Suggestions resume next word” meanwhile.

| Context is cleared by | Suggestions are held |
|---|---|
| A key that failed to type, a failed acceptance, Retry predictions, or a worker replaced after a missed deadline | When a word is in progress |
| A shortcut | When a word is in progress |
| An arrow key, Home, End, Page Up, Page Down, Delete or another key that is not typed text | Always, because the caret is then in text Switchify never saw and is taken to be inside a word |
| Backspace that deletes text the worker does not have, such as text that was there before the keyboard opened | When the character before the caret is part of a word or is unknown |
| Enter or Tab | Never; a new line or field begins |
| A foreground change, or keyboard and mouse activity from outside Switchify | Never; typing continues somewhere else. These also end a hold |

Backspace releases a hold once it has deleted back to a space or punctuation Switchify typed. A hold costs at most the suggestions for one word; it never changes what is typed. To know whether a word is in progress, the main process remembers for each character Switchify typed only whether it was part of a word, never the character, for at most 512 characters. While the activity observer is not running, outside activity cannot be seen and does not end a hold, and neither does the observer starting again. Changing the switch key assignments counts as outside activity.

A passive observer records only an activity counter and timestamp, never external text. Prediction is unavailable if this observer cannot start or loses access. Edits made before the observer is ready are discarded because intervening activity cannot be verified. Each queued edit is scoped to its foreground target and the time before injection, so edits preceding an observed external change are discarded. Changes within an application that produce no observed input cannot be detected without inspecting its fields.

## On-device model

One Word prediction setting controls the English `en-aac-oanc-v1` SQLite database through the pinned `switchify-prediction` Rust library. The saved `enhancedWordPrediction` field remains compatible but does not choose an engine. Only the isolated worker opens the read-only baseline; there is no personal database, saved learning, import, or runtime download.

The worker ranks up to five words using interpolated trigram/bigram/unigram probabilities (0.6/0.3/0.1, renormalized when context is unavailable). Suggestions begin with the first typed character and continue after a space as next-word suggestions. An untouched keyboard remains empty. The prefix and preceding text are supplied separately, and sentence punctuation resets language context. There are no generated phrases. Unknown prefixes yield no suggestions. The vocabulary can contain names, disfluencies and inappropriate words; it is English-only and is not a clinical recommendation system.

Loading is asynchronous and typing remains available. The existing badge distinguishes loading, waiting for typed context, held suggestions, no match, available suggestions, paused activity tracking, and failure. **Retry predictions** replaces the worker without closing the keyboard. A clipped buffer without a complete earlier word yields no suggestions. Repeated calls exceeding 1.5 seconds mark prediction unavailable; a missed two-second reply deadline triggers the existing worker replacement policy. Errors never expose buffered text.

The loader checks the pinned database SHA-256 before opening it. Missing, corrupt or incompatible resources fail closed. `npm run prediction-model` obtains the versioned upstream release, verifies its archive and individual file hashes, and installs only allowlisted resources after verification. Verified local files work without network access. Bundles carry the source manifests, attribution and separate corpus notices; see [model provenance](../src-tauri/resources/PROVENANCE.md).

## Casing

The label on a suggestion always reads as the text will after it is accepted. Casing is decided in three steps.

**1. The word itself.** Database lookup words are normalized lowercase. The pronoun I and its standard contractions (I'm, I'll, I'd and I've) receive a capital I for display. Proper-name capitals are not guessed; use Shift or Caps to enter them.

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

Accepting restores a capital the typed prefix lacks by backspacing that prefix and typing the whole word, as in “i” for I’m. When the case already matches, which is the usual case, nothing is deleted and only the missing suffix is typed. A capital the person typed is kept, and a word typed in capitals is never retyped.

The deletion is bounded. The prefix is text Switchify typed itself in the current window with no outside activity since. It is never more than one word, and the executor refuses more than 32 deletions. Any keyboard or mouse activity not made by Switchify between choosing the suggestion and typing it cancels the acceptance, so nothing else is deleted.

Two things cannot be guaranteed. Deleting and typing cannot be atomic: if typing fails after the deletion, the prefix is lost and prediction context resets. The deletion also assumes one Backspace removes one typed character; an application that auto-pairs or autocorrects what was typed can break that, and Switchify cannot verify what an application accepted.

The database is not committed. `npm run prediction-model` downloads the pinned release bundle and verifies its SHA-256. The Tauri dev and build commands and CI run it automatically. Run it once before `cargo test` or `cargo clippy` in a fresh checkout.

## Worker process

A separate process owns the buffer, activity observer and model. Private bounded inherited pipes carry successful edits and results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. When a reply misses it, the worker is killed and reaped with its text context and a new worker starts by itself, so suggestions return after the model loads. If a word was in progress, suggestions resume with the next word. A second missed deadline within 60 seconds of that replacement stops prediction and offers Retry predictions, whatever the replacement answered in between. So does a third missed deadline while the same keyboard is open, however far apart they are. Reopening the keyboard or selecting Retry predictions starts the count again. An acceptance that was waiting on the missed reply fails and types nothing; the keyboard shows its error state for that acceptance while prediction itself recovers. Ordinary keyboard operation remains available throughout. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

### Spare worker

Loading remains lazy on first keyboard use, so a keyboard that had a working worker leaves a spare behind when it closes. The worker that held the typed text is killed and reaped first. A new process is then started, which loads the model and waits. It has received no request, so it holds no text, and it starts its activity observer only on its first request, so it observes nothing while it waits. The next keyboard open adopts it and suggestions are ready without a reload.

At most one spare exists, and only between keyboard opens. It is killed and reaped after two minutes without a keyboard open, whenever scanning ends or restarts, such as after saving settings, when Word prediction is turned off, on Retry predictions and on exit. A spare started for different switch keys, or one that has exited, is discarded and a new worker is started instead. A keyboard whose worker failed leaves no spare. In each of these cases, and on the first open of a scanning session, the next open loads the model again. A keyboard reopened before loading completes adopts a spare that is still loading and shows the loading badge until it finishes. While it waits, the spare holds the loaded model in memory. CI records platform-specific load time and memory measurements rather than assuming a fixed allocation.

Before accepting a suggestion, the worker checks its token, edit revision, foreground identity and external activity. The main process checks its generation and foreground again before injection. Verification and native insertion cannot be atomic across applications; the target can still change in that short interval.

## Validation

Automated tests use fake predictors, foreground/activity sources, input adapters and sleeping subprocesses. They never inject desktop input. They cover first-letter completion, Unicode Backspace, bounded context, stale replies, foreground/external changes, failure cleanup and switch action compatibility. `cargo test` exercises the SQLite adapter with synthetic corpora. Packaged model verification and isolated release-mode measurements run separately on macOS and Windows. For release-build suggestions and timings:

```powershell
cargo test --release --lib --manifest-path src-tauri/Cargo.toml bundled_prediction_benchmark -- --ignored --nocapture --test-threads=1
```

For native validation, use disposable synthetic text in Notepad and a browser on Windows, and TextEdit and a browser on macOS. Launch macOS with `npm run macos:run` to retain its signed Accessibility identity.

1. Open the keyboard from the action menu and an assigned Open keyboard switch, including from idle, a paused scan and an active drag. Verify no click or focus change occurs and owned drag input is released.
2. Type `w`, then `a`, accept `water`, and continue typing. Existing or pasted text must never become prediction context.
3. Change pages and docking, type punctuation and numbers, and return to Letters. Verify context survives these layout changes and Backspace edits the tracked buffer.
4. Use external keyboard/mouse activity. Verify suggestions clear and a new first letter starts fresh context. Then type `hel`, press an arrow key, and type `l`: verify no suggestions appear and the badge reads “Suggestions resume next word” until a space is typed. Repeat with Retry predictions and with a shortcut in place of the arrow key.
5. Change foreground apps with locked modifiers selected. Verify the keyboard stays open, the modifiers stay locked, suggestions clear, and later keys go to the new app.
6. Close the keyboard, stop scanning, disconnect and exit. Verify input releases and the prediction worker exits. After closing the keyboard one spare worker remains; reopen after a few seconds and within two minutes, and verify the loading badge clears almost immediately, then verify the spare exits after two minutes idle and when scanning stops. Test observer failure separately; typing should remain usable without predictions.

Compilation and fake-adapter tests do not establish native application compatibility. Record live results separately.

### Database adapter measurement

On the development Apple M2 Max (12 logical CPUs), an isolated release-mode
adapter test loaded the verified 29,802,496-byte database in 1,758 ms. Across
200 warmed queries, p50 was 0.254 ms and p95 was 5.812 ms. Process RSS rose from
10.0 MiB to 371.5 MiB; this includes the Rust test harness and retained allocator
memory, and is not a measurement of the whole desktop application. The SQLite
file size does not describe its in-memory n-gram representation.

Keep first-use loading asynchronous and lazy: preloading at app startup would
allocate this memory even for people who never open the keyboard. The existing
bounded spare-worker policy handles subsequent opens. Native CI records the same
measurements against extracted Windows and macOS package resources; timing is
reported, not enforced as a flaky shared-runner threshold. See the integration
PR for each platform's report.
