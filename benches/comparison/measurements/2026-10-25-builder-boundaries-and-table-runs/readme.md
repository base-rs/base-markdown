# Consolidate semantic text state and boundary scans

This study compares main `74e6fa5d90ba58125200220c8bb79152adaef25a` with the implementation at `4e9a45a`. The first five changes were also measured separately at `6d649a3` (the **stage5** cohort). Public APIs, rendered output, authored syntax, source spans and allocation traffic are preserved.

## Implementation

- Replace two independently optional text-builder fields with one existing `Text` state. It adopts owned buffers and retains the old append-growth policy for small borrowed/literal fragments. Compiler layout on this machine shrinks from 64 to 40 bytes.
- Scan reference-label brackets in reverse, examining each bracket's preceding escape run once. The former reverse byte loop recounted prefixes within long backslash runs and was quadratic.
- Share one-terminal-ending calculations across source endpoints and semantic slices. Frontmatter no longer depends on the table parser for this rule.
- Copy ordinary UTF-8 table-cell runs together between pipe escapes, preserving backslash parity and code-tick matching. This uses the existing `memchr` dependency.
- Reuse the first email `@` search result without narrowing the backward local-part boundary.
- Move the existing physical line-ending iterator into the shared utility module. List-item spacing inspects only the first or second required ending and stops at the first spread child pair, instead of counting all endings and checking all subsequent pairs.

An attempted removal of explicit empty-list-item state was rejected: a nested empty item changed the outer item's authored end offset while producing identical HTML. The state remains; the discovered CR/LF/CRLF boundary case is now covered. No public features or parser guarantees are removed.

## Measurements

All timings use the system allocator on an Apple M4 Pro with AArch64 macOS. Seven alternating processes per engine were used in each stage5 cohort; final cohorts use the round counts recorded in their summaries. Each process warms for 100 ms and measures a calibrated approximately 120 ms batch, with matched iteration counts within each round. Both engines use the same launch path. No study builds, tests, diagnostics or allocation measurements overlap timing. Every process sample is retained, including host outliers; cohorts are kept separate rather than pooled. This is a shared workstation: an unrelated compilation was observed during the final ordinary cohort, including heavy load on its first original-example pairs. Those pairs have very wide spreads and support no precise performance conclusion. A separate seven-round repeat of the four original examples is retained; later runs had lighter background load, without machine isolation.

Direct HTML includes parsing, output allocation, rendering and disposal. Prepared HTML and Markdown exclude document construction and include fresh output allocation and disposal. `syntax` walks a prepared document's descendants and black-boxes each inferred `NodeSyntax`. Prepared operations are not whole-pipeline parser benchmarks. The driver black-boxes output strings before observing their lengths. Timing enables default crate features; allocation and serde-enabled diagnostics use separate binaries. HTML permits raw HTML and all protocols; the GFM preset disables footnotes for continuity with the original comparison. Per-input metadata enables frontmatter or heading IDs where specified.

The stage5 ordinary cohort shows small movements in both directions. The focused cases isolate the intended scan savings: long Unicode table-cell Markdown takes about 69% less time, email HTML with a long ordinary prefix takes about 17% less, and the 800-backslash reference workload drops from about 12.8 ms to 0.37 ms for prepared Markdown. These targeted inputs do not establish comparable gains for ordinary documents. Inkmark was not retimed in this study.

The separate original-example repeat has direct HTML changes from −1.5% to +0.6%, prepared HTML from −3.0% to −1.1%, and Markdown from −1.8% to −0.4%. These small movements support retaining the consolidation; they do not establish a universal speedup.

| Operation / workload | Baseline µs | Final µs | Time change | Spread B / F |
| --- | ---: | ---: | ---: | ---: |
| html / readme | 13.50 | 13.30 | -1.49% | 2.0% / 1.0% |
| ast-html / readme | 8.37 | 8.22 | -1.74% | 1.5% / 1.3% |
| markdown / readme | 63.92 | 63.69 | -0.36% | 0.8% / 2.2% |
| html / mixed-100k | 396.96 | 399.29 | +0.59% | 1.9% / 2.5% |
| ast-html / mixed-100k | 263.86 | 259.96 | -1.48% | 2.6% / 2.8% |
| markdown / mixed-100k | 1400.42 | 1378.34 | -1.58% | 1.5% / 3.9% |
| html / mixed-1m | 4045.38 | 4031.57 | -0.34% | 1.5% / 1.9% |
| ast-html / mixed-1m | 2605.73 | 2528.99 | -2.95% | 3.1% / 3.5% |
| markdown / mixed-1m | 13869.67 | 13672.00 | -1.43% | 4.0% / 3.1% |
| html / gfm-100k | 605.28 | 601.24 | -0.67% | 2.7% / 2.3% |
| ast-html / gfm-100k | 378.01 | 374.00 | -1.06% | 3.6% / 2.7% |
| markdown / gfm-100k | 1578.50 | 1550.07 | -1.80% | 2.7% / 3.0% |

Final focused repeats:

| Operation / workload | Baseline µs | Final µs | Time change | Spread B / F |
| --- | ---: | ---: | ---: | ---: |
| markdown / reference-labels-slash-heavy-800 | 12647.31 | 356.69 | -97.18% | 35.6% / 5.7% |
| syntax / reference-labels-slash-heavy-800 | 12650.48 | 44.95 | -99.64% | 25.8% / 31.8% |
| markdown / tables-long-unicode-far-pipes | 124.98 | 38.43 | -69.25% | 6.3% / 2.4% |
| html / email-long-prefix-valid | 121.20 | 100.53 | -17.06% | 1.1% / 3.1% |
| html / email-long-prefix-no-marker | 72.83 | 72.66 | -0.23% | 0.5% / 1.0% |

Final list-spacing fixtures (prepared syntax inspection, not parsing):

| Operation / workload | Baseline µs | Final µs | Time change | Spread B / F |
| --- | ---: | ---: | ---: | ---: |
| syntax / list-blocks-8 | 20.36 | 16.27 | -20.08% | 1.0% / 0.8% |
| syntax / list-blocks-32 | 81.67 | 53.64 | -34.32% | 2.3% / 1.8% |
| syntax / list-gap-65536 | 25.05 | 0.04 | -99.83% | 0.9% / 9.2% |
| markdown / list-gap-65536 | 0.39 | 0.39 | +0.69% | 1.1% / 4.1% |

Elapsed-time change is `100 × (candidate / baseline − 1)`; negative is faster. Spread is `100 × (max / min − 1)` across process samples, not a confidence interval. The complete summaries and compressed raw measurements retain controls as well as gains:

- Stage5: [ordinary](ordinary-summary.json), [focused](focused-summary.json), [reference syntax](reference-summary.json).
- Final: [ordinary](final-ordinary-summary.json), [list spacing](final-lists-summary.json), [focused repeats](final-focused-summary.json), [original-example repeat](final-repeat-summary.json).

## Correctness and allocations

The [final preservation comparison](preservation-final.json) agrees on all 9,535 inputs: four original examples, six pinned real documents, 51 focused cases, six list-spacing fixtures and 9,468 generated container/boundary cases. Comparisons include direct and AST HTML, Markdown, heading values, resolved parser/document events, inferred authored syntax and source ranges. Semantic text storage is normalized while its resolved content and authored syntax ranges are compared. The generated cases are a regression probe, not a proof for all possible Markdown.

All 536 final allocation records agree exactly with baseline on allocation count, reallocations, cumulative requested bytes and operation result across 67 inputs and four operations. The raw `output_bytes` field holds output length for renderers and the inferred-syntax count for `syntax`. [Allocation summaries](final-allocations-summary.json) retain each pair; [raw counts](final-allocations.json.gz) come from separate instrumented executables. Cumulative requested bytes are allocation traffic, not peak memory. The smaller builder is stack state and does not imply a heap-allocation reduction.

Formatting, all-target/all-feature workspace Clippy, native all-feature coverage and full workspace tests pass. Coverage is 14,135/14,135 measured source regions, with 21 invariant-only unreachable exclusions. [Validation metadata](validation.json) records commands and log hashes. CI separately exercises Linux. Public APIs did not change, so an additional downstream release/test train is unnecessary for this internal pass.

## Reproduction

[Implementation metadata](implementation-metadata.json) records compiler/OS/CPU, source-file hashes, frozen executable hashes and driver/lock identities. Inputs are archived as `inputs-*.json.gz`; generated container probes are reproduced by `preserve.py`. The scripts reuse the preceding study's pinned inputs, lockfile, builder and process runner.

```sh
STUDY=benches/comparison/measurements/2026-10-25-builder-boundaries-and-table-runs
PREVIOUS=benches/comparison/measurements/2026-10-25-deferred-values
WORK=/tmp/core-markdown-boundary-study
# Supply separate checkouts of baseline 74e6fa5 and final 4e9a45a.
python3 -B "$STUDY/build.py" --baseline /path/to/baseline --candidate /path/to/final --work "$WORK"
python3 -B "$STUDY/preserve.py" --work "$WORK" --candidate candidate
python3 -B "$PREVIOUS/run.py" \
  --binary baseline="$WORK/baseline-bench" --binary final="$WORK/candidate-bench" \
  --data "$WORK/data/original.json" --data "$WORK/data/real.json" \
  --rounds 5 --out "$WORK/final-ordinary.json"
python3 -B "$PREVIOUS/run.py" \
  --binary baseline="$WORK/baseline-bench" --binary final="$WORK/candidate-bench" \
  --data "$WORK/data/lists.json" --mode syntax markdown \
  --rounds 7 --out "$WORK/final-lists.json"
python3 -B "$PREVIOUS/run.py" \
  --binary baseline="$WORK/baseline-bench" --binary final="$WORK/candidate-bench" \
  --data "$WORK/data/original.json" --rounds 7 --out "$WORK/final-repeat.json"
python3 -B "$PREVIOUS/run.py" \
  --binary baseline="$WORK/baseline-allocations" --binary final="$WORK/candidate-allocations" \
  --data "$WORK/data/original.json" --data "$WORK/data/real.json" \
  --data "$WORK/data/focused.json" --data "$WORK/data/lists.json" \
  --mode html ast-html markdown syntax --alloc --out "$WORK/final-allocations.json"
```

To reproduce stage5, use a `6d649a3` candidate checkout. The summary metadata identifies the exact datasets, cases, modes, process rounds and executable hashes for each separate cohort.
