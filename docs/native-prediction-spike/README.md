# Native prediction spike (#797 / draft #798)

## Recommendation

Keep both providers experimental. Mac is the stronger candidate for a later production trial: this VM produced relevant UK-English completions and next words with modest process overhead. Windows also returned usable results after the locale retry/filtering path, but needs broader language-pack and quality coverage. Neither is qualified for the shipping default: switch-driven insertion and suggestion-row timing have not been validated interactively in this run, and the six-sentence corpus is too small to establish quality or reliability.

The implementation is stacked on PR #796, `codex/word-prediction-795`, at `e5ee2a598b062640d311ae8e6c45de2729127b7d`. The VM binaries and raw results below were built from implementation commit `90bde1d507d08d783e07b2ea966bff06c01e0db7`. No schema, BLE, settings UI, or provider selector changes are included. Native-first is the default only in this experimental branch when the existing word-prediction setting is enabled.

## Measurements

2026-09-19, ARM64 VMs: macOS 26.6.2 (25G83), Windows 11 Pro 10.0.26200. Each mode ran six fixed synthetic contexts five times. Each native request starts the same executable in native-helper mode, measures IPC and process startup as well as the native API, then measures the database on the same context. These are warm-machine debug-build measurements, not cold-boot or release benchmarks. Each query uses a fresh helper; identical-context caching is deliberately bypassed by the comparison command.

| Platform / mode | Queries | Empty usable results | Native errors | DB fallback | Native median / p95 / max (ms) | DB median / p95 (ms) |
|---|---:|---:|---:|---:|---|---|
| mac / online | 30 | 0 | 0 | 0 | 52.6 / 61.0 / 93.1 | 0.38 / 18.16 |
| mac / offline | 30 | 0 | 0 | 0 | 54.2 / 59.6 / 60.1 | 0.56 / 18.22 |
| windows / online | 30 | 0 | 0 | 0 | 148.2 / 151.4 / 157.8 | 0.68 / 21.92 |
| windows / offline | 30 | 0 | 0 | 0 | 148.5 / 167.9 / 178.5 | 0.65 / 24.19 |

Raw, synthetic-only measurements: [Mac online](mac-online.jsonl), [Mac offline](mac-offline.jsonl), [Windows online](windows-online.jsonl), [Windows offline](windows-offline.jsonl). `fallback_used` means the filtered native list was empty/error and would choose the database in the shared provider; the comparison additionally queries the database even when native succeeds. No real editor content is recorded.

| Synthetic context | Mac native, first three | Windows native, first three | Database, first three |
|---|---|---|---|
| `hel` | hello, help, helps | hello, help, helping | held, help, helped |
| `wat` | watch, watching, water | watching, watch, water | water, waters, watch |
| `I want some wa` | water, warm, way | water, way, want | way, ways, water |
| `Thank you ` | for, so, again | for, so, have | very, for, so |
| `How are ` | you, your, we | you, u, things | you, we, they |
| `I would like ` | to, a, that | to, that, it | to, you, the |

The native results offer useful contextual next words in this small sample. The database remains much faster for simple prefixes. These examples do not establish an accuracy score: there is no labelled intended-word corpus, and colloquial/capitalized native candidates may be inappropriate for some users. Locale-specific standalone exploration before the integrated run found empty UK lists on this Windows installation and usable US lists; the committed integrated measurements do not log which locale supplied each result.

For the offline runs, each VM's Parallels `net0` was disconnected and an external HTTPS request to `example.com` failed with curl error 6 (DNS resolution). All 60 offline native queries still returned usable results. Both adapters were reconnected afterward. Local VM-to-host transfer remained reachable. This verifies operation during the tested external-connectivity failure; it is not a packet-capture audit or a guarantee about every OS service's networking behaviour.

## Reproduction

Build this branch with Node 24 and Rust 1.97.1. Do not run the application or input tests on the user's signed local Mac installation. Copy a built app into a separate VM test directory.

Mac, from a normal logged-in VM Terminal (the helper needs a native application session):

```sh
app='/path/to/test/Switchify PC.app'
"$app/Contents/MacOS/switchify-pc" \
  --switchify-native-prediction-probe \
  "$app/Contents/Resources/resources/WordData2017051601.db" \
  online > mac-online.jsonl
```

Windows ARM64, from the normal interactive user's PowerShell after `npm run tauri -- build --debug --no-bundle --target aarch64-pc-windows-msvc`:

```powershell
& .\src-tauri\target\aarch64-pc-windows-msvc\debug\switchify-pc.exe `
  --switchify-native-prediction-probe `
  .\src-tauri\resources\WordData2017051601.db online |
  Set-Content windows-online.jsonl
```

Adjust the executable path if `CARGO_TARGET_DIR` is set. Supply the existing database explicitly. Require exactly 30 JSON rows; an empty/truncated file is a failed run. Disconnect only the test VM network, verify external connectivity fails, repeat with `offline`, and restore networking in a `finally`/cleanup step. This probe does not type, extract foreground text, change permissions, or enable scanning. Its explicit synthetic output is separate from the production worker, which does not log text.

The debug application resolves its prediction database from the build-time source directory. For the Mac VM app-launch attempt, the bundled database was mirrored to that path inside the guest. The standalone comparison does not depend on that workaround because it accepts an explicit database path.

## Implementation and automated validation

`Provider` separates native generation from the existing database and shared context/insertion engine. Native work runs in a disposable helper process with a 600 ms budget inside the existing two-second prediction-worker deadline. Windows contains children in a Job; Mac helpers watch the parent. Cancellation still terminates the outer worker and invalidates pending batches. Native failure, timeout and empty filtered lists use the database. Unsafe/protected, selected, stale-focus and invalid contexts remain rejected by the shared engine before any provider request; the target and input generation are checked again after generation.

Mac supplies the surrounding text and UTF-16 prefix range to NSSpellChecker, selecting an available `en_GB` dictionary or `en`. A zero-length range at a boundary returned next words in this VM. Apple's API documentation describes partial-word completion, so the observed zero-length behaviour should be requalified across supported macOS versions. [Apple completion API](https://developer.apple.com/documentation/appkit/nsspellchecker/completions(forpartialwordrange:in:language:inspelldocumentwithtag:))

Windows explicitly selects `TextPredictionOptions::Predictions`, tries UK then US after unusable/empty/error UK results, and uses `GetNextWordCandidatesAsync` at a boundary. Previous words are passed to the candidate API. [Microsoft candidate API](https://learn.microsoft.com/en-us/uwp/api/windows.data.text.textpredictiongenerator.getcandidatesasync?view=winrt-26100)

Filtering preserves rank, removes duplicate/empty/multiline/incompatible words, bounds results, and only accepts words compatible with suffix-only insertion. The shared cache preserves unchanged batch tokens, so identical results do not request a new suggestion layout or restart scan timing.

Fake-provider/input tests cover rank, filtering, native-first behaviour, database failure/fallback, locale retry, invalid context, identical-result caching, exactly-once suffix-and-space insertion, and killing a hanging helper before the overall worker deadline. Existing prerequisite tests cover shared context validation, cancellation, focus/input-generation changes, worker timeout, protected text, and scanning behaviour. These tests are not a substitute for native interactive qualification.

Required local frontend lint/tests/build and Mac Rust format/Clippy/tests passed (449 unit tests, seven integration tests; one manual benchmark ignored). Windows ARM64 Clippy and Rust tests passed; the native overlay smoke test was excluded from the headless SYSTEM-session test runner. The Windows Tauri debug build and Mac development-signed app build succeeded. The signed local Mac app was not stopped or replaced.

## Interactive qualification gap and restoration

Both spike apps launched in the VMs and displayed Ready/input access ready using temporary manual-scan bindings. Initial Mac Space attempts were inconclusive because manual scan lines started at the display edge. A follow-up using J for Select, K for Next and L for Previous successfully opened the action menu and keyboard, scanned to h and inserted it once (`hel` became `helh`), inserted a Space after deleting that h, and reached and activated Close keyboard using switches. TextEdit retained focus for those insertions. However, the suggestion row remained empty for both `hel` and the word boundary, despite the standalone native helper returning useful candidates. This is an unresolved integrated context/worker/provider-path blocker; its origin in #796 or this spike has not been isolated. Suggestion acceptance, repeated switch characters, focus/selection/protected-field cases and stable live suggestion-row timing remain **unqualified**. In Windows, the computer-control path did not reliably deliver even unrelated typing or clicks to the disposable Notepad document, including after Switchify was stopped. Do not mark either platform's complete interactive prediction gate passed.

Before a production decision, repeat the full planned switch-driven matrix with reliable VM input: complete a prefix, insert next words, check capitalization/trailing spaces and repeated letters; retain editor focus; change selection/focus and enter protected fields; observe stable row timing; repeat offline. Include cold starts, slower systems, other supported OS versions and Windows language-pack configurations. Keep database fallback and explicit empty/error handling.

The two settings files were backed up and restored byte-for-byte in each VM. Windows's prior #796 executable was restarted, without overwriting its binary. Mac had no Switchify process before testing; the spike was stopped and the VM returned to its prior suspended state. Network connections were restored, no new Accessibility grants or security-policy changes were made, and the Mac disposable editor windows were closed. The Windows disposable Notepad tab may remain open because UI input was unavailable; existing user tabs were preserved. No native keyboard visibility was changed.

Keep the PR draft until these findings and the outstanding interactive gates are reviewed. Retarget to main only after #796 merges. Merge/release and any shipping-default change need separate instructions.
