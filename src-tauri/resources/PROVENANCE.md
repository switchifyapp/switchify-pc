# English prediction database

`WordData2017051601.db` was recovered from the user's private `enaboapps/sayit-ios` history, path `Assets/Databases/WordData2017051601.db`, commit `95bda9265758e50787c5da2779b16f36b53c319d`, dated May 20, 2017. Recovery took place September 18, 2026. The user authorized bundling this database with Switchify.

- Size: 102,690,816 bytes.
- SHA-256: `dedd65d263bde8315e7e5ed7d2c8e04f17c33598a68506c9d70661a6d1f57318`.
- SQLite integrity check during recovery: `ok`.
- Rows: WORDS 165,420; BIGRAMS 976,804; TRIGRAMS 996,846; QUADGRAMS 953,630.

This is the unchanged original SQLite file. The later Realm archive is not used. It is retained for conversion and parity tests, not shipped in the app. The converter ignores personal frequency columns. No captured text or user vocabulary is written into this resource.

## Packaged dedicated lookup

`word-predictions.lookup` is generated from the unchanged database. It contains frequency-ranked words and n-grams, a sorted prefix index, a versioned header and a SHA-256 payload checksum. The runtime validates counts, bounds, references and ordering before serving predictions. The app packages only this lookup and does not write to it.

Regenerate with Rust 1.97.1 from the repository root:

```sh
cargo run --release --locked --manifest-path tools/prediction-data/Cargo.toml -- src-tauri/resources/WordData2017051601.db src-tauri/resources/word-predictions.lookup
```

Use `-- --check` before the two paths to regenerate into a temporary file and verify byte-for-byte reproducibility. CI checks the artifact on Mac and Windows. The original SQLite implementation is retained only in tests to verify identical ordered suggestions.
