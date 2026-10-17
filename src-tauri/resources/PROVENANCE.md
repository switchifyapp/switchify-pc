# English prediction baseline

Switchify PC uses `switchify-prediction` v0.4.0 at commit
`858cef433c1e42a1bd6e95774c519aea1aed9abb` and model `en-aac-oanc-v1`.
The Rust dependency is pinned by commit and Cargo.lock. The baseline is the
n-gram file `english.ngrams`, which contains 21,674 words and 539,151 n-grams.
Its SHA-256 is
`c6df388c845354a059333b1e4abe68719e5a7016e8514c44581e14ed16bf964a`.

`npm run prediction-model` downloads the [versioned release](https://github.com/switchifyapp/switchify-prediction/releases/tag/v0.4.0)
and verifies the archive and each shipped resource against
`scripts/prediction-model.json`. No raw training files or personal data are bundled.
The installed app uses the baseline offline and read-only. No ONNX runtime,
model, tokenizer or third-party native-runtime download is used.

New library code is MIT licensed; SQLite, used for the personal learning file, is public domain. Corpus licensing is
separate: WorldAlphabets identifies its English Tatoeba CC0 source; Taskmaster
and AAC are CC BY 4.0. OANC's current publisher grants unrestricted use and
redistribution, including commercial use; its historical XML-release notice has
different restrictions. Both notices remain included, and no independent legal
clearance is claimed.

The application bundles ATTRIBUTION.md, LICENSE, production-model.json,
source-manifest.json, aac-source-manifest.json and corpus-notices alongside the
baseline. Retain these when redistributing. The corpus includes crowd-imagined
AAC communication and older American speech; suggestions can be inappropriate
or incorrect. Model quality results are available in the upstream release.
