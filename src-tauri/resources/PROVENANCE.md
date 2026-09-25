# Word prediction model

`prediction-model/` is not committed. `npm run prediction-model` (`scripts/fetch-prediction-model.mjs`) downloads it from [`onnx-community/SmolLM2-135M-ONNX`](https://huggingface.co/onnx-community/SmolLM2-135M-ONNX) at revision `d0ae6834f1df45e0e95b5fdae95e536f9ca7cd3f` and verifies each file's size and SHA-256 before use:

- `model.onnx`: upstream `onnx/model_int8.onnx`, 135,658,354 bytes, SHA-256 `50ba80511ce74634d232a043b6c37775cca756b826b49d0a4a8eff958c4bbcc9`.
- `tokenizer.json`: 2,053,526 bytes, SHA-256 `139d2f4b4919b90953bdd3c0c40c94c9b23074799a766508dc3bf5eb8ab73351`.
- `config.json`: 1,035 bytes, SHA-256 `2c5f23fddabecdf9c47d0048f555899822ec87ec4a28169393840ac7e74192c4`.

It is an int8 ONNX conversion of [HuggingFaceTB/SmolLM2-135M](https://huggingface.co/HuggingFaceTB/SmolLM2-135M), licensed Apache-2.0. The app bundles these files unchanged and only reads them.

# ONNX Runtime

Word prediction statically links ONNX Runtime 1.28.0 through the `ort` crate (`=2.0.0-rc.13`, MIT/Apache-2.0). At build time, `ort-sys` downloads the prebuilt archive for the target from `cdn.pyke.io` and verifies it against a SHA-256 embedded in the crate. So every build, including signed releases, depends on that download. Only the CPU provider is used, and ONNX Runtime telemetry is disabled when the worker starts it.
