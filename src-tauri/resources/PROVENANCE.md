# English prediction database

Switchify PC uses `switchify-prediction` v0.1.0 at commit
`c4b9d14ba8312179617ead4d2707e86ec825a2e6` and model `en-aac-oanc-v1`.
The Rust dependency is pinned by commit and Cargo.lock. The baseline contains
21,674 words and 539,151 n-grams. Its SHA-256 is
`222253417d0a7a705823ffb7e599a3bcf5d5d3daf4a9d76161ac6b3e555aeaad`.

`npm run prediction-model` downloads the [versioned release](https://github.com/switchifyapp/switchify-prediction/releases/tag/v0.1.0)
and verifies the archive and each shipped resource against
`scripts/prediction-model.json`. No raw training files or personal data are bundled.
The installed app uses the database offline and read-only. No ONNX runtime,
model, tokenizer or third-party native-runtime download is used.

New library code is MIT licensed; SQLite is public domain. Corpus licensing is
separate: WorldAlphabets identifies its English Tatoeba CC0 source; Taskmaster
and AAC are CC BY 4.0. OANC's current publisher grants unrestricted use and
redistribution, including commercial use; its historical XML-release notice has
different restrictions. Both notices remain included, and no independent legal
clearance is claimed.

The application bundles ATTRIBUTION.md, LICENSE, production-model.json,
source-manifest.json, aac-source-manifest.json and corpus-notices alongside the
database. Retain these when redistributing. The corpus includes crowd-imagined
AAC communication and older American speech; suggestions can be inappropriate
or incorrect. Model quality results are available in the upstream release.
