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

# Enhanced prediction model

`prediction-model/` is not committed. `npm run prediction-model` (`scripts/fetch-prediction-model.mjs`) downloads it from [`onnx-community/SmolLM2-135M-ONNX`](https://huggingface.co/onnx-community/SmolLM2-135M-ONNX) at revision `d0ae6834f1df45e0e95b5fdae95e536f9ca7cd3f` and verifies each file's size and SHA-256 before use:

- `model.onnx`: upstream `onnx/model_int8.onnx`, 135,658,354 bytes, SHA-256 `50ba80511ce74634d232a043b6c37775cca756b826b49d0a4a8eff958c4bbcc9`.
- `tokenizer.json`: 2,053,526 bytes, SHA-256 `139d2f4b4919b90953bdd3c0c40c94c9b23074799a766508dc3bf5eb8ab73351`.
- `config.json`: 1,035 bytes, SHA-256 `2c5f23fddabecdf9c47d0048f555899822ec87ec4a28169393840ac7e74192c4`.

It is an int8 ONNX conversion of [HuggingFaceTB/SmolLM2-135M](https://huggingface.co/HuggingFaceTB/SmolLM2-135M), licensed Apache-2.0. The app bundles these files unchanged and only reads them.

# Prediction blocklist

`prediction-blocklist.txt` is the unchanged English list from [LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words](https://github.com/LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words) (`en` at commit `4638b970cb8d9d82789564fcba1f4a1eb508ff1a`, 403 lines, SHA-256 `af851ecef1d5f212caba17339b12ac39cc2fef7d78c74876f67237644fcee8bd`). It is licensed CC BY 4.0 by its contributors. Enhanced prediction never suggests a listed word.

# ONNX Runtime

Enhanced prediction statically links ONNX Runtime 1.28.0 through the `ort` crate (`=2.0.0-rc.13`, MIT/Apache-2.0). At build time, `ort-sys` downloads the prebuilt archive for the target from `cdn.pyke.io` and verifies it against a SHA-256 embedded in the crate. So every build, including signed releases, depends on that download. Only the CPU provider is used, and ONNX Runtime telemetry is disabled when the worker starts it.
