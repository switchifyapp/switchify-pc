# Word prediction runtime

Switchify PC predicts words with the n-gram model from
[TextSlinger](https://github.com/kdv123/textslinger), run by a self-contained
runtime built by [switchify-prediction](https://github.com/switchifyapp/switchify-prediction).

`npm run prediction-model` downloads the runtime for the build target from the
pinned switchify-prediction release and checks its size and SHA-256 against
`scripts/textslinger-runtime.json` before unpacking it into `textslinger/runtime`.
`textslinger/bridge.py` is this repository's helper that the app runs with it.
The installed app never downloads anything; the runtime and model are used
offline and read-only.

The runtime contains standalone CPython 3.10, numpy, regex, KenLM, textslinger
and the `lm_dec19_char_tiny_12gram` model. The model is redistributed with the
TextSlinger authors' permission. Each component's licence is in the runtime's
`notices` folder or in its package's `.dist-info` folder; `notices/NOTICE.md`
lists them. Retain these when redistributing. KenLM is LGPL-2.1-or-later, and its
unchanged source is included as `notices/kenlm/kenlm-0.2.0.tar.gz`.

Suggestions can be inappropriate or incorrect. No accuracy is claimed.
