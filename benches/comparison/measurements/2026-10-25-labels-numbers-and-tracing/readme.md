# Shared Markdown formatting and allocation-free HTML numbers

This study compares PR #61 at `0e6a3cf5b9489dbd31f98e5cce6fe1a47bca578f` with production/test commit `09224baa56e781def69f6e4cac88f7ca73c4e554`. Separate commits share Markdown delimiter/link-label formatting, format HTML numbers in a stack buffer, and remove obsolete tracing plumbing.

Successful output and diagnostics are preserved. The no-op `log` feature is intentionally removed: callers must stop requesting `features = ["log"]`. The example no longer initializes `env_logger`; removing that development dependency prunes 21 packages from the workspace lockfile, with no dependency upgrades.

## Implementation

Emphasis, strong and strikethrough share delimiter wrapping. Resource/reference links and images share label rendering while retaining their distinct following contexts and option-check order. The shared helpers use the same output buffer and add no temporary allocations.

The HTML writer formats ordered-list attributes and footnote counters into 20-byte stack buffers. The decimal formatter handles zero and the entire `u64` range; supported targets' `usize` counters fit that range. Repeated backlinks format once and reuse the result for both the ID suffix and superscript. Numbers still pass through the existing output routing, preserving captures, last-written-byte tracking and writer errors. No runtime dependencies are added.

## Performance

The original four examples range from 0.6% less to 0.9% more HTML time and 0.2% less to 0.6% more Markdown time. These small differences do not establish a meaningful change. Focused serializer controls are within 1%; the repeated-footnote control takes 14.1% less time, 256 notes take 4.2% less, and 256 separate ordered lists take 2.8% less. Inkmark was not retimed for this cleanup.

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.012836 | 0.012861 | +0.20% | 0.3% / 2.0% |
| html/mixed-100k | 0.385783 | 0.385187 | -0.15% | 0.8% / 1.3% |
| html/mixed-1m | 3.863404 | 3.840045 | -0.60% | 0.8% / 3.0% |
| html/gfm-100k | 0.594138 | 0.599767 | +0.95% | 0.7% / 1.3% |
| markdown/readme | 0.061484 | 0.061432 | -0.08% | 0.8% / 1.1% |
| markdown/mixed-100k | 1.354333 | 1.361677 | +0.54% | 0.8% / 1.1% |
| markdown/mixed-1m | 13.590365 | 13.567000 | -0.17% | 0.7% / 1.0% |
| markdown/gfm-100k | 1.552386 | 1.561787 | +0.61% | 2.3% / 1.3% |

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| controls/delimited-256 | 3.134736 | 3.122365 | -0.39% | 2.2% / 0.9% |
| controls/resources-256 | 0.358849 | 0.355703 | -0.88% | 1.0% / 2.1% |
| controls/references-256 | 0.591653 | 0.590829 | -0.14% | 0.3% / 2.7% |
| features/ordered-lists-256 | 0.064366 | 0.062593 | -2.75% | 1.3% / 1.3% |
| features/footnote-single | 0.000957 | 0.000939 | -1.92% | 0.4% / 1.9% |
| features/footnote-repeated-64 | 0.011062 | 0.009506 | -14.06% | 1.3% / 0.9% |
| features/footnotes-256 | 0.382889 | 0.366883 | -4.18% | 1.1% / 2.0% |

Changes are `100 × (candidate / baseline − 1)`; negative values mean less time. Spread is `(maximum process median / minimum process median − 1) × 100`, not a confidence interval. All 24 process/mode records, 90 document/process results and 630 batch samples are retained in [raw timings](primary.jsonl). The [summary](primary-summary.json) includes all process medians. No samples were discarded and no second timing cohort was run.

## Output and allocation checks

The [established preservation report](preservation.json) compares 8,468 corpus outputs, 58,596 diagnostic probes, 120 complete edited arenas, original direct/checked/document HTML and Markdown outputs, and 23,449 additional serializer/footnote records. [Focused checks](focused-preservation.json) cover the seven new workloads. All compared outputs and diagnostics agree byte for byte.

[Focused allocation records](focused-allocations.json) show no additional allocations from the serializer helpers. The 256 ordered lists avoid 256 number-string allocations, a single note avoids one, 64 repeated calls avoid 190, and the 256-note/three-calls workload avoids 1,792. Requested bytes fall by 2,304, 1, 300 and 3,004 respectively. Original four-example allocation counts are unchanged. [Established allocation checks](allocations.json) retain another 63 records. Counts include one warmed operation using separate instrumented executables; requested bytes are cumulative allocation/growth requests, not peak or retained memory.

Native coverage passes 13,833/13,833 measured regions, with 21 invariant-only unreachable exclusions. Workspace tests, all-feature/all-target Clippy and formatting pass. [Validation](validation.json) records commands, source identities and log hashes. Independent review is required before opening the PR; Linux runtime checks are left to CI.

## Method and reproduction

Apple M4 Pro, AArch64 macOS, Rust 1.99.1/LLVM 23.1.3. Both source revisions were built and frozen using the unchanged historical build driver. [Metadata](metadata.json) pins source revisions, binary hashes, compiler, adapters, fixtures and lockfiles. Three sequential alternating rounds reverse engine order in round two. Each document warms for 200 ms, then runs seven calibrated approximately 150 ms batches. Tables use the median of three process medians. HTML includes parsing, rendering and output disposal. Markdown/serializer controls serialize prepared documents and include output disposal. Startup, fixture loading, options and AST preparation for serialization are excluded. Builds, tests and diagnostics completed before timing. The host remained shared without affinity or isolation.

```sh
python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/prepare.py
python3 benches/comparison/measurements/2026-10-25-labels-numbers-and-tracing/prepare.py
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/build.py \
  --baseline /path/to/pr61 --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-labels-numbers-and-tracing/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/primary.jsonl \
  --mode html --mode markdown --mode controls --mode features \
  --input controls=benches/comparison/measurements/2026-10-25-labels-numbers-and-tracing/serializer.json \
  --input features=benches/comparison/measurements/2026-10-25-labels-numbers-and-tracing/numbers.json
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py --input /tmp/study/primary.jsonl
```

Use the recorded revisions, compiler and lockfiles when reproducing the measurements. Corpus attribution remains in the preceding studies. Generated fixtures are ignored and hashed; frozen executables remain outside Cargo target directories.
