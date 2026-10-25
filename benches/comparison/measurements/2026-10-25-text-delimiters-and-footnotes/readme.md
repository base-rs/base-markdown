# Editable text, delimiter selection, and footnote lookup

This study compares squash-merged #60 with three changes. Public APIs, features, dependencies, successful output and diagnostic behavior are unchanged. Inkmark was not retimed.

- `Text::to_mut` owns source/literal materialization and reuses existing owned strings. `Document::text_mut` retains frontmatter handling and propagates invalid-range errors, including source buffers with interior mutability.
- Inline-code and math delimiter selection records short marker-run lengths in one machine word and collects only longer lengths during the same scan. Short-run contents avoid the temporary allocation and sort; long runs can still allocate even if a shorter delimiter is available. Sorting is needed only when every bit is occupied. There is no repeated input scan, and exact delimiter choices are preserved.
- A typed footnote-call record caches its escaped ID and final definition association. Shared indexing helpers index only called labels. One walk matches definitions in completion order and stops once all calls resolve; rendering retains first-use order. Empty and nested duplicate definitions retain the first completed body. Call lookup while consuming events remains linear in distinct called labels.

This is a small consolidation and optimization pass, rather than a net code removal: production sources grow by about 40 physical lines after excluding test-only items; this includes comments and blank lines.

## Sources and timing

Baseline is `174813e0fe63a57005c1582a43649c4b57248a62` (#60). Final production/test commit is `7ace4c156ab61b05351712067b421159a21a6aa6`, source tree `b0810c8cd90c07227973f5c64d24883ffb1d17a4`. Separate commits cover text materialization, delimiters and footnotes. [Metadata](metadata.json) pins sources, frozen binaries, fixture/driver hashes, compilers and locks.

All times are milliseconds; lower is faster. Changes are relative to baseline. Original HTML takes **0.7–2.3% less time**; original Markdown ranges from **0.9% less to 1.0% more time**. These small effects should not establish a universal ranking. Sparse short-marker controls take **about 30% less time**; repeated-footnote and 256-note controls take **11.4% and 9.2% less time** respectively. The dense synthetic delimiter controls with lengths 1–1,024 take **4.5–4.9% more time**, and lengths 1–4,096 take **1.7–1.9% more time**. This measurable cost is retained alongside the common short-run allocation reduction; dense work still uses one scan and bounded sorting. Few-call/many-definition controls range from **2.1% less to 1.9% more time**.

### Original four workloads

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.013106 | 0.012906 | -1.52% | 0.8% / 0.5% |
| html/mixed-100k | 0.392122 | 0.383083 | -2.31% | 1.1% / 1.0% |
| html/mixed-1m | 3.922375 | 3.870295 | -1.33% | 1.5% / 1.3% |
| html/gfm-100k | 0.600900 | 0.596959 | -0.66% | 2.5% / 0.4% |
| markdown/readme | 0.061092 | 0.061024 | -0.11% | 2.0% / 0.6% |
| markdown/mixed-100k | 1.365212 | 1.352440 | -0.94% | 1.0% / 2.7% |
| markdown/mixed-1m | 13.591413 | 13.529069 | -0.46% | 0.3% / 2.3% |
| markdown/gfm-100k | 1.547935 | 1.563946 | +1.03% | 1.0% / 1.3% |



### Delimiter controls

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| controls/code-plain | 0.000116 | 0.000116 | +0.11% | 1.1% / 4.4% |
| controls/code-sparse | 0.001135 | 0.000790 | -30.37% | 2.9% / 0.9% |
| controls/code-dense-64 | 0.001337 | 0.001228 | -8.09% | 1.1% / 0.7% |
| controls/code-dense-1024 | 0.156408 | 0.163374 | +4.45% | 0.4% / 0.5% |
| controls/code-dense-4096 | 2.694950 | 2.746016 | +1.89% | 0.7% / 4.3% |
| controls/code-many-small | 1.233122 | 1.232039 | -0.09% | 0.6% / 1.5% |
| controls/math-plain | 0.000118 | 0.000118 | -0.70% | 1.6% / 3.5% |
| controls/math-sparse | 0.001161 | 0.000806 | -30.58% | 4.3% / 2.3% |
| controls/math-dense-64 | 0.001339 | 0.001233 | -7.90% | 0.2% / 6.0% |
| controls/math-dense-1024 | 0.156533 | 0.164217 | +4.91% | 0.8% / 4.3% |
| controls/math-dense-4096 | 2.696657 | 2.741327 | +1.66% | 0.6% / 0.4% |
| controls/math-many-small | 1.240932 | 1.234976 | -0.48% | 1.2% / 0.5% |



### Footnote and reference controls

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| features/footnote-single | 0.000944 | 0.000952 | +0.90% | 1.7% / 1.4% |
| features/footnote-repeated-64 | 0.012615 | 0.011179 | -11.39% | 3.7% / 2.9% |
| features/footnotes-256 | 0.418643 | 0.380001 | -9.23% | 1.0% / 2.4% |
| features/footnote-blocks-and-options | 0.001950 | 0.001840 | -5.62% | 5.1% / 1.7% |
| features/references-4096 | 0.619341 | 0.628011 | +1.40% | 1.3% / 0.5% |
| features/duplicate-reference-and-footnote-keys | 0.197057 | 0.182465 | -7.40% | 1.7% / 0.6% |
| features/footnotes-unused-1 | 0.000391 | 0.000391 | +0.18% | 1.3% / 1.9% |
| features/footnotes-unused-256 | 0.057225 | 0.057436 | +0.37% | 0.2% / 0.2% |
| features/footnotes-before-calls-256 | 0.320843 | 0.292192 | -8.93% | 1.3% / 2.3% |



### Few calls among many definitions

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| features/footnotes-1-calls-256-definitions | 0.059936 | 0.060736 | +1.33% | 0.6% / 0.9% |
| features/footnotes-2-calls-256-definitions | 0.060732 | 0.061893 | +1.91% | 1.2% / 1.7% |
| features/footnotes-4-calls-256-definitions | 0.062065 | 0.063264 | +1.93% | 0.2% / 0.3% |
| features/footnotes-16-calls-256-definitions | 0.068887 | 0.068496 | -0.57% | 2.1% / 2.0% |
| features/footnotes-1-calls-4096-definitions | 0.973503 | 0.974395 | +0.09% | 0.9% / 1.8% |
| features/footnotes-2-calls-4096-definitions | 0.980092 | 0.993460 | +1.36% | 0.8% / 0.9% |
| features/footnotes-4-calls-4096-definitions | 0.996852 | 0.994131 | -0.27% | 1.6% / 2.9% |
| features/footnotes-16-calls-4096-definitions | 1.055505 | 1.033131 | -2.12% | 0.6% / 1.8% |



The [primary samples](primary.jsonl), [targeted samples](targeted.jsonl) and [few-call samples](sparse-calls.jsonl) retain all 30 completed process/mode records and 222 document/process results; no final samples were discarded.

HTML measurements include parsing, rendering and output disposal. Markdown and delimiter controls serialize prepared documents and include output disposal. Options and fixture construction are outside timing. The text-mutation refactor has correctness coverage but no isolated timing claim.

## Allocations and preservation

[Allocation records](allocations.json) count one warmed operation in separate instrumented executables; timing uses the native allocator. Requested bytes are cumulative allocation/growth requests, not peak or retained memory. All result lengths/counts match.

The 256-small-span controls each avoid 256 allocations. Sparse short-run controls avoid the run-length allocation and growth. Long dense controls retain their existing allocation counts. A 64-call footnote avoids 127 allocations; the 256-note/three-calls control avoids 1,535 allocations. Called records retain cached IDs and a definition association: the single-note control requests 184 more bytes, and the 256-note control requests 10,740 more bytes (about 1.2%). The repeated-call control requests 320 fewer bytes. Unused-note controls have identical allocation counts and requested bytes to baseline. These changes do not establish a universal memory reduction.

[Preservation results](preservation.json) compare 8,468 corpus outputs, 58,596 diagnostic probes, the original direct/checked/document HTML and Markdown outputs, and 120 complete edited-arena snapshots. Another 23,449 complete records cover existing serializer output/errors, dense/gapped code/math delimiters and footnote controls. Public regressions cover source/literal mutation and owned-buffer reuse, first-call order, first-completed empty/nested duplicate footnotes, contextual Unicode lowercase IDs and renderer reuse after a call writer fails at every output boundary.

Native all-feature/all-target coverage passes 13,812/13,812 measured regions, with 21 invariant-only unreachable exclusions. Workspace tests, all-feature/all-target Clippy, formatting and Linux x86_64 all-feature compilation pass. Linux verification here is compile-only; CI runs runtime tests and coverage. [Validation](validation.json) records the source identities and check-log hashes.

## Rejected experiments

The [map prototype](exploratory-map/metadata.json) kept one canonical map record per footnote label. It requested approximately 0.8–0.9 KiB more for tiny cases and made 256 unused notes about 28% slower. Its delimiter fallback rescanned dense contents and made the large dense controls roughly 75–95% slower. Completed process records are retained; one running process was interrupted after observing the regression.

The [definition-index prototype](exploratory-definition-index/metadata.json) fixed the delimiter scan and retained cheap definition appends, but indexed every definition at completion. One/few calls among 256/4,096 definitions took roughly 10% more time. The final batch lookup indexes called labels instead. Both experiments retain complete source/test patches against the baseline, source/binary identities and completed samples; final measurements use separate binaries and cohorts.

## Method and reproduction

Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. Each cohort uses three alternating sequential rounds, reversing engine order in round two and launching fresh executable inodes. Each document warms for at least 200 ms, then runs seven batches calibrated to approximately 150 ms. Results are medians of three process medians; process spread is `(maximum / minimum - 1) × 100`, not a confidence interval. All completed final process/batch results are retained. Startup, JSON and fixture setup are outside timing. Own builds, tests, diagnostics and reviews ran outside final timing; unrelated host processes remained active, without affinity/isolation.

The [generator](prepare.py), [verifier](verify.py) and [recorder](record.py) reuse the unchanged historical feature adapter, calibration, allocation counter, corpus inputs and drivers. Generated large fixtures are ignored and hashed. Corpus attribution and licenses remain in the preceding studies. Frozen executables stay outside Cargo target directories.

```sh
python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/prepare.py
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/primary.jsonl --mode html --mode markdown
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/targeted.jsonl --mode controls --mode features \
  --input controls=benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/delimiter-controls.json \
  --input features=benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/features.json
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/sparse-calls.jsonl --mode features \
  --input features=benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/sparse-calls.json
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/primary.jsonl --input /tmp/study/targeted.jsonl --input /tmp/study/sparse-calls.jsonl
```

Use the recorded source revisions, lockfiles and compiler to reproduce saved results. Recording validation and rejected experiments also requires their logs and historical source objects; timing and output comparison can run independently of the recorder.
