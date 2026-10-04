# Six prediction slots

Slots 1–3 are the statistical predictor's first three suggestions. Slots 4–6 are
additional SmolLM2 words. Empty slots stay empty and cannot be selected in either
scan direction. The entire displayed row stays stable until scanning leaves it.

The worker retains exact displayed/latest batch tokens and validates acceptance
against the edit revision, generation, activity epoch and foreground target.
Capitalization, prefix replacement and trailing spaces use the existing path.
No focused-field capture, personal learning or new settings are introduced.

The companion uses protocol 2. Model and tokenizer bytes are unchanged. Startup
remains bounded to 30 seconds and each reply to two seconds. Failure leaves the
instant slots available, with explicit retry or keyboard reopening required.

## Release assets

The build downloads checksum-pinned v0.2.1 companion release archives from
GitHub. Both libraries and worker provenance are pinned to the release commit.
Verified local caches work offline, and installed applications need no network
access or model setup. Model, tokenizer and statistical database bytes remain
unchanged.

## Validation

Run the repository checks and scripts/measure-neural.py --build with explicit
--model, --worker and --output paths. The integration fixture uses synthetic
context and fake input/activity adapters. It measures 1,000 warmed queries,
asserts instant-slot stability, and reports fill, latency, failures and process
tree memory. Its overall measurement bound is 40 minutes; the application reply
deadline remains two seconds.

Use switchify-prediction scripts/generation_evaluate.py for the frozen corpus
comparison. Existing model quality regressions remain. Neither default activation
nor a successful performance run establishes unseen-data accuracy.

## Initial comparison

A deterministic 1,000-query sample of the 1,760-query frozen development/test
workload produced the following results. These runs measured implementation
70fc4a8; final companion c25f068 changes release versions and documentation only.
Generation starts with a fresh context cache for each measured query.

| Metric | Windows portable | macOS ARM |
| --- | ---: | ---: |
| Existing reranker top-three | 50.1% | 50.3% |
| Existing reranker top-six, at most five returned | 51.8% | 52.0% |
| Six statistical top-three | 39.9% | 39.9% |
| Six statistical top-six | 48.2% | 48.2% |
| Six-slot top-three | 39.9% | 39.9% |
| Six-slot top-six | 72.6% | 72.7% |
| Neural slot fill | 93.5% | 93.4% |
| Correct OOV query instances | 29/63 | 29/63 |
| Gains / regressions against six statistical | 260 / 16 | 260 / 15 |
| Generation median / p95 | 1152 / 1388 ms | 349 / 767 ms |
| Generation maximum | 1671 ms | 1701 ms |
| Peak process-tree RSS | 658 MiB | 862 MiB |

Both runs completed 1,000 generation queries without generation failures. The
macOS comparison reranker failed once and was explicitly retried. Windows had
no failures. Local Windows development checks overlapped part of measurement;
final CI repeats qualification on dedicated runners. The comparison covers a
sample of the frozen workload, not the whole corpus. Generated OOV occurrences
were 702 and 701 respectively, counting repeated query occurrences.

Keeping the statistical first three sacrifices the previous reranker's
first-three accuracy. Total top-six accuracy improves on this sample, but some
queries regress. The existing quality qualification failures remain; unknown
training overlap prevents claims about unseen-data accuracy.

Reproduce in the companion checkout with Python dependencies psutil==7.0.0 and
regex==2025.11.3, using explicit paths to the prepared offline assets:

```text
python scripts/generation_evaluate.py --cli <companion-cli> --baseline <english.sqlite> --bundle <model-directory> --worker <portable-worker> --samples 1000 --output <report.json>
```

Omit --samples to evaluate all 1,760 queries. The companion CI uploads platform
JSON reports as generation-windows-latest and generation-macos-latest. Desktop
CI separately checks packaged resources and measures the production integration
with fake input adapters. Those measurements are distinct from CLI generation.

The extracted Windows installer also completed 1,000 production-engine queries
with its optimized worker and no failures or retries. Immediate median/p95/max
were 8.34/9.35/15.25 ms; generation was 496/537/1035 ms. All 3,000 neural slots
filled on this synthetic fixture, and peak process-tree RSS was 664 MiB. This
measurement includes the engine, model adapter and child IPC with 20 ms polling.
It excludes the outer desktop pipe and rendering and does not establish corpus
accuracy. Installer contents and offline loading were verified separately.

The measured JSON reports are checked in under docs/measurements/six-predictions.
Their worker and corpus hashes identify the exact inputs. Final-head CI reports
are additional evidence and may vary with runner hardware and timing cutoffs.
