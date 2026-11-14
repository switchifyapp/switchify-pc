# Word prediction

Word prediction is enabled by default in Scanning settings. Its five-position row appears only on the Letters page. Empty positions are skipped. Select the row and then a word using the existing switches. Acceptance appends the missing suffix and a space without selecting text or using the clipboard. It deletes nothing, with one bounded exception: when the word has a capital where a lowercase letter was typed, as in “i” for I’m, it backspaces the typed prefix and types the whole word. Shift and Caps affect completion casing; Ctrl, Alt/Option and Windows/Command suppress suggestions.

Predictions use only a temporary buffer of successful Switchify keyboard input, starting with the first letter. Existing text, pasted text and hardware keyboard typing are never read into it. Switchify does not inspect fields, selections, passwords or caret positions. Suggestions may therefore appear anywhere the keyboard is open, including password fields or applications without a text field. The buffer records successful input injection; it cannot verify what an application actually accepted.

The buffer holds at most 512 characters, retaining complete Unicode graphemes at its leading boundary. It tracks ordinary characters, spaces, Backspace and accepted completions across keyboard pages and top/bottom docking. Navigation, Delete, Enter, Tab, shortcuts, failed input, external typing/clicks/scrolling and foreground changes clear context. Closing the keyboard, ending scanning, disconnecting and exiting also discard it, by killing the process that held it. The keyboard remains open across foreground changes, resets modifiers and suggestions, and sends subsequent input to the new foreground application.

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

## TextSlinger

Suggestions come from the n-gram model in [TextSlinger](https://github.com/kdv123/textslinger), run by the bundled runtime described in [the bundled TextSlinger runtime](textslinger-runtime.md). The worker starts the runtime's Python with `resources/textslinger/bridge.py` and asks it for up to five completions of the word being typed, given the text before it. The bridge lowercases the context, and only English letters and apostrophes are completed: a prefix with digits, accents or other characters, and a non-space tail with no prefix, return no suggestions without stopping the model. The worker keeps only replies that are plain lowercase completions of the typed prefix. A suggestion whose accepted text would exceed the 64 characters the executor types is not offered.

The model does not learn, correct mistyped letters or suggest phrases. It can suggest non-words, or the word already typed, in which case accepting adds only a space. Earlier versions could learn from typing into `prediction/personal.sqlite` in this computer's app data. Switchify deletes that file and its journals when it starts.

## Casing

The label on a suggestion always reads as the text will after it is accepted. Casing is decided in three steps.

**1. The word itself.** TextSlinger's words are lowercase, so neither the pronoun I nor names are capitalised on their own; use Shift or Caps to enter them.

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

The runtime is not committed. `npm run prediction-model` downloads the pinned runtime and verifies its SHA-256. The Tauri dev and build commands and CI run it automatically. Run it once before `cargo test` or `cargo clippy` in a fresh checkout; the tests that need the runtime say so and pass without it.

## Worker process

A separate process owns the buffer, activity observer and model. Private bounded inherited pipes carry successful edits and results to native rendering. Text and suggestions are not sent to the React UI, diagnostic history or telemetry. One request is outstanding at a time with a two-second deadline. When a reply misses it, the worker is killed and reaped with its text context and a new worker starts by itself, so suggestions return after the model loads. If a word was in progress, suggestions resume with the next word. A second missed deadline within 60 seconds of that replacement stops prediction and offers Retry predictions, whatever the replacement answered in between. So does a third missed deadline while the same keyboard is open, however far apart they are. Reopening the keyboard or selecting Retry predictions starts the count again. An acceptance that was waiting on the missed reply fails and types nothing; the keyboard shows its error state for that acceptance while prediction itself recovers. Ordinary keyboard operation remains available throughout. Closing the keyboard, ending scanning, or exiting kills and reaps the worker.

### Loading before the keyboard opens

The model loads in a spare worker before the keyboard is first opened. Once scanning starts, while the keyboard is closed and Word prediction is on, the main process starts one spare worker, once per scan session. When it has put the worker in its kill-on-close job (Windows) or process group (macOS), it sends a warm-up request, and only then does the worker start the runtime, so the runtime can never outlive it. A warm-up gets no reply and does not start the activity observer. The spare has received no text, so it holds none. Opening the keyboard adopts it, so suggestions are usually ready at once.

Closing the keyboard kills and reaps the worker that held the typed text, then starts a new spare for the next open. A spare waits as long as the app runs: it uses memory for the runtime even while the keyboard stays closed, which is the cost of not waiting for it on each open. It is killed and reaped whenever scanning ends or restarts, such as after saving settings, when Word prediction is turned off, on Retry predictions and on exit. A spare started for different switch keys is not adopted. If the spare cannot start, the keyboard starts its own worker when it opens, and no spare is retried until then.

The runtime has a 90-second allowance to load, for a cold disk or an antivirus scan of its files. Each request has 1.2 seconds within the worker's two-second reply deadline. A missed reply, an invalid reply or a helper that exited makes the model unavailable for that worker; the keyboard shows Retry predictions, and there is no automatic retry loop. A request that takes the helper more than 30 seconds makes it exit; idle waiting has no limit.

Before accepting a suggestion, the worker checks its token, edit revision, foreground identity and external activity. The main process checks its generation and foreground again before injection. Verification and native insertion cannot be atomic across applications; the target can still change in that short interval.

## Validation

Automated tests use fake predictors, foreground/activity sources, input adapters and sleeping subprocesses. They never inject desktop input. They cover first-letter completion, Unicode Backspace, bounded context, stale replies, foreground/external changes, failure cleanup and switch action compatibility. Tests in `src-tauri/src/prediction/worker/tests/textslinger.rs` also run the bundled runtime with fixed text, and synthetic faulty peers from `scripts/textslinger-bridge-test.py`.

For native validation, use disposable synthetic text in Notepad and a browser on Windows, and TextEdit and a browser on macOS. Launch macOS with `npm run macos:run` to retain its signed Accessibility identity.

1. Start scanning, wait a few seconds, then open the keyboard from the action menu and an assigned Open keyboard switch, including from idle, a paused scan and an active drag. Verify suggestions are available almost at once, and no click or focus change occurs.
2. Type `w`, then `a`, accept `water`, and continue typing. Existing or pasted text must never become prediction context.
3. Change pages and docking, type punctuation and numbers, and return to Letters. Verify context survives these layout changes and Backspace edits the tracked buffer.
4. Use external keyboard/mouse activity. Verify suggestions clear and a new first letter starts fresh context. Then type `hel`, press an arrow key, and type `l`: verify no suggestions appear and the badge reads “Suggestions resume next word” until a space is typed.
5. Change foreground apps with locked modifiers selected. Verify the keyboard stays open, the modifiers stay locked, suggestions clear, and later keys go to the new app.
6. Close the keyboard, stop scanning, disconnect and exit. Verify input releases and the worker and its runtime process exit, apart from the one spare worker the closed keyboard leaves.

Compilation and fake-adapter tests do not establish native application compatibility. Record live results separately.
