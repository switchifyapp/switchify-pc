# Nearby-key layout benchmark

This engineering benchmark uses 30 invented contexts and target words with the
verified production English n-gram model. It does not capture input, start the
desktop app or use personal learning. Its results do not establish user benefit.

Prepare resources with `npm run prediction-model`. Commit the source, then run:

```powershell
python scripts/benchmark-nearby-layouts.py --output docs/nearby-key-results.json
```

The runner requires a clean checkout, builds the release-profile library test,
uses the executable Cargo reports, and records its hash without publishing local
paths. `CARGO_TARGET_DIR` is supported. The report records the measurement commit,
fixture/model/source hashes, compiler and platform. Source hashes use committed
Git blobs with LF line endings. The runner checks that the compiled source hashes
match the working files and that those files differ from the commit only in line
endings. Each policy/group receives 20 warm-up queries. Timings are descriptive.

The three maps come from the renderer's built-in QWERTY, Simple QWERTY and Common
Letters frames with default settings and no custom tiles. Centers share one
horizontal key pitch as their unit. Radius 1.4 and spread 0.45 are starting values,
not calibrated probabilities. Only the 26 lowercase English letters are mapped.

For prefixes of two through four letters, every mapped geometric neighbor is
substituted at each position. Separate scan-row cases choose the previous or
next built-in row at the same scanner column. Non-letter outcomes are excluded.
Cases are deduplicated and hashed per layout and error group. These cases do not
model actual switch timing or the frequency of different errors.

Three top-three policies use the same contexts and cases: ordinary exact-prefix
prediction, the library's weighted hit-map ranking, and a conservative policy
that preserves exact suggestions and fills only unused instant slots. The last
policy leaves empty and one-letter prefixes exact. Empty and one-to-four-letter
latency probes are included separately. Neural inference, UI, transport and text
insertion are outside this measurement. Runtime prediction behavior is unchanged
by this benchmark PR.

## Recorded run

The checked-in JSON was measured at `8a07fe050f13ea732e4a14b727f0fb059a8c2805` with the release profile. The fill-unused-slots policy retained every exact top-three hit in these fixtures and recovered targets in each layout?s geometric-error group. This is a policy check on invented cases, not a user accuracy result. The JSON contains the complete counts and separate scan-row results.
