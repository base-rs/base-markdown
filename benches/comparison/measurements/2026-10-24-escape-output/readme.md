# HTML escape-output measurements

The implementation is commit `52713d10b6ce4d1979dbbacee5101d4f014c3396`, compared with merged main `0e3bdd08fc1481704967712df5e4ac1937a7d7ec` (PR #37). The baseline executable is byte-identical to the previous study's final candidate, and its production source and tests match the squash-merged main. [Metadata](metadata.json) records compiler, runtime, machine, source/executable hashes, and input hashes. The original four benchmark inputs are unchanged.

## Retained change

The shared renderer scans for the first escape before entering its replacement loop. A span without replacements uses the existing single-write path. A span needing replacements selects the active heading capture, footnote capture, or caller's writer once, then emits unchanged runs and replacements directly. It skips empty runs around replacements and preserves the last successfully written byte when a caller's writer fails. The replacement helper returns the writer's small error type and converts it to a stream error at the renderer boundary.

There are no new allocations, dependencies, SIMD instructions, public event changes, or removed features. Heading capture still takes precedence over footnote capture. Raw HTML still replaces NUL while preserving other characters; ordinary text still escapes NUL, ampersand, quote, and angle brackets. The [screening record](exploratory/readme.md) retains the less effective variants and every sample.

## Complete HTML rendering

Times below are milliseconds per document: the median of five process medians, each containing seven calibrated samples after a 200 ms warmup. Baseline/candidate/Inkmark launch order alternates. Rust revisions use the same launch path, arguments, and environment. Builds, tests, profiles, and independent reviewer workloads did not overlap timing measurements. The machine was not otherwise isolated. All samples, including outliers, are retained in the [manifest](run-manifest.json) and [summary](summary.json).

| Workload | Merged main | Candidate | Inkmark 0.2.1 | Candidate time reduction vs main |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 0.01487 | 0.01449 | 0.01897 | 2.6% |
| Mixed 100 KB | 0.43591 | 0.42054 | 0.41172 | 3.5% |
| Mixed 1 MB | 4.41814 | 4.26536 | 4.42031 | 3.5% |
| GFM 100 KB | 0.64544 | 0.63618 | 0.71735 | 1.4% |
| References | 0.62592 | 0.60283 | 0.85637 | 3.7% |
| Long prose 1 MB | 0.12408 | 0.11814 | 0.59897 | 4.8% |
| Fenced code 1 MB | 0.07435 | 0.07050 | 0.32275 | 5.2% |

The candidate takes 3.5% less time than Inkmark on mixed-1m in this run, but still takes 2.1% more time on mixed-100k. The overall-fastest objective remains unmet. CommonMark benchmark outputs agree after entity decoding, not byte-for-byte. **GFM output differs in literal links and table serialization**, so that engine comparison is not an equal-output ranking; see [output agreement](benchmark-output-agreement.json).

The first baseline mixed-1m process measured 9.33 ms; later baseline processes measured 4.39–4.44 ms. Candidate round two measured 4.47 ms, while its other rounds measured 4.26–4.27 ms. Candidate README round one and baseline README round four also have conspicuous slow samples. No samples were removed, and no cause was established. The five-process medians reduce sensitivity to these outliers, but do not eliminate background activity or compiler/process-layout effects. These measurements support a modest gain, not a universal speed ranking. Definitions at three sizes are single-process regression observations rather than repeated rankings.

## Checked events and arena rendering

These modes use three alternating baseline/candidate pairs. Checked rendering includes parsing, public event validation, HTML output, and disposal. Arena mode includes parsing, arena construction, traversal, HTML output, and disposal. They are independent complete operations; do not subtract them from direct rendering times to infer phase costs.

| Workload | Mode | Main ms | Candidate ms | Time reduction |
| --- | --- | ---: | ---: | ---: |
| Mixed 100 KB | Checked events to HTML | 0.49721 | 0.48680 | 2.1% |
| Mixed 1 MB | Checked events to HTML | 4.99724 | 4.91910 | 1.6% |
| Mixed 100 KB | Arena parse and render | 1.14342 | 1.14676 | -0.3% |
| Mixed 1 MB | Arena parse and render | 12.42305 | 12.40921 | 0.1% |

Mixed arena performance is essentially unchanged. [Phase summary](phase-summary.json) retains all process medians, including README and GFM.

## Validation and reproduction

All [allocation records](allocation-agreement.json), including reallocations, cumulative requested bytes, event counts, AST node counts, and node size, match main. All [2,117 saved HTML outputs](preservation.json) are byte-identical through direct, checked-event, and arena rendering. Focused tests cover adjacent escapes and vector boundaries, Unicode, NUL/raw HTML, headings nested in footnotes, and continuation after failures at different writer positions.

Formatting, all-feature workspace Clippy, workspace tests, and local arm64 all-feature/all-target coverage pass: **26,870/26,870 measured regions**, with 68 invariant-only exclusions. The library also cross-compiles for x86_64 Linux. No architecture-specific code changed. [Validation](validation.json) records these checks.

Use the [comparison harness guide](../../readme.md), restore the recorded source revisions and toolchain, and build the comparison executables. The archived Python drivers retain session-specific baseline/output paths; substitute local paths while retaining one common launch path, argument list, and environment for both Rust revisions. `final-bench.py` records raw timings and can resume completed invocations; `summarize.py` recomputes phase medians and verifies input hashes and allocation agreement. `preserve.py` and `agreement.py` reproduce output checks using the saved original corpus and pinned Inkmark installation.
