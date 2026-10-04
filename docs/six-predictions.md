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

## Prepared assets

The build can consume checksum-pinned companion CI archives before publication,
using an authenticated GitHub CLI. Verified local caches work offline. These CI
artifacts expire and are not permanent release assets. Before merging/releasing
the desktop, separately authorize companion artifact publication and replace
prepared artifact sources with the permanent release URLs for the same bytes.
No runtime network access or model setup is needed.

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
