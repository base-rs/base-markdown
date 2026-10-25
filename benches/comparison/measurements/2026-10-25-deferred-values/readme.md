# Preserve source-backed values through rendering

This study compares released main `0ec8b512e35ebf8c79acd897379a3915e341f727` with implementation `be85ff306a4705a99ab13cc174115ff58a06526e`. Four separate optimization commits remove unnecessary conversions and copies without adding parser state, dependencies, public APIs or node fields. The changed production sections contain one fewer physical line in total; most of the diff is regression coverage.

## Implementation

- Retain image titles as semantic `Text` until the image closes, then use the shared escaping writer. Source ranges and literals need no title-string allocation; custom borrowed values are copied before their producer releases them. Remove the separate `encode_into` wrapper.
- Ignore empty source fragments when continuing a semantic value. Leading escapes can leave the remainder source-backed; entirely empty values retain their initial offset, and noncontiguous nonempty fragments still materialize.
- Share borrowed semantic text extraction with setext-heading selection. Its nonempty and literal-break checks use one early-exiting traversal instead of constructing the heading's text. Code-block and synthetic empty-break distinctions remain intact.
- Leave rendered table cells without pipe bytes in the output buffer, skipping scratch rewriting. Cells with pipes retain the existing escaping algorithm.

## Timing

The original direct-HTML examples remain within 0.5% of baseline in the primary cohort. Six pinned real documents range from −1.6% to +1.1% for direct HTML, −1.7% to +1.0% for prepared HTML, and −1.2% to +1.4% for prepared Markdown. Small changes in that range are not evidence of a broad speedup. Inkmark was not retimed.

| Operation / workload | Baseline µs | Candidate µs | Time change | Spread B / C |
| --- | ---: | ---: | ---: | ---: |
| html / readme | 13.26 | 13.23 | -0.21% | 1.6% / 1.3% |
| ast-html / readme | 8.20 | 8.14 | -0.73% | 2.2% / 2.7% |
| markdown / readme | 62.95 | 62.59 | -0.58% | 1.0% / 1.0% |
| html / mixed-100k | 390.06 | 391.24 | +0.30% | 2.8% / 3.2% |
| ast-html / mixed-100k | 256.29 | 257.42 | +0.44% | 22.1% / 2.9% |
| markdown / mixed-100k | 1379.98 | 1395.55 | +1.13% | 3.6% / 2.7% |
| html / mixed-1m | 3941.84 | 3925.93 | -0.40% | 1.1% / 2.7% |
| ast-html / mixed-1m | 2531.34 | 2575.37 | +1.74% | 2.0% / 2.0% |
| markdown / mixed-1m | 13736.45 | 13583.30 | -1.11% | 3.4% / 3.7% |
| html / gfm-100k | 601.97 | 602.65 | +0.11% | 2.6% / 1.3% |
| ast-html / gfm-100k | 372.06 | 372.82 | +0.20% | 2.7% / 2.0% |
| markdown / gfm-100k | 1581.99 | 1568.91 | -0.83% | 2.6% / 1.7% |

The focused cases show the intended benefits: image HTML takes 4.7–11.6% less time, escaped metadata takes 23.7% less, heading serialization takes 8.3–11.9% less, and table serialization takes 2.5–6.1% less.

| Operation / workload | Baseline µs | Candidate µs | Time change | Spread B / C |
| --- | ---: | ---: | ---: | ---: |
| html / images-simple | 319.60 | 294.65 | -7.81% | 4.1% / 3.3% |
| ast-html / images-simple | 203.33 | 179.73 | -11.61% | 8.0% / 5.2% |
| html / images-rich-alt | 656.42 | 625.86 | -4.66% | 3.3% / 3.0% |
| ast-html / images-rich-alt | 342.86 | 324.79 | -5.27% | 2.6% / 22.3% |
| html / metadata-leading-escapes | 551.93 | 421.18 | -23.69% | 2.3% / 2.8% |
| ast-html / metadata-leading-escapes | 203.69 | 204.75 | +0.52% | 3.1% / 3.4% |
| markdown / headings | 2023.90 | 1856.98 | -8.25% | 3.0% / 2.2% |
| markdown / tables-no-pipes | 906.99 | 851.51 | -6.12% | 6.4% / 5.2% |
| markdown / tables-pipe-heavy | 924.57 | 901.24 | -2.52% | 4.7% / 5.7% |
| markdown / headings-forced-setext | 1972.82 | 1738.27 | -11.89% | 4.0% / 4.2% |
| markdown / headings-multiline | 2133.83 | 1939.21 | -9.12% | 3.0% / 6.6% |

These are relative elapsed-time changes, `100 × (candidate / baseline − 1)`: negative means less time. Spread is `(max / min − 1) × 100` over process samples, not a confidence interval. The [raw primary results](primary.json.gz) retain all 840 process samples across 60 workload/operation pairs; the [summary](primary-summary.json) includes the complete results, including controls without a targeted gain.

## Selective repeat

The same frozen executables repeat the original four workloads, the Rust NLL RFC, title/definition metadata and the heading-ID case. Direct HTML on the original four remains within 0.6% of baseline. The 1 MB prepared-HTML sample takes 1.0% longer (1.7% in the primary cohort); that small cost is retained alongside the focused gains. Title/definition Markdown takes 1.3% longer (1.7% primary). Heading-ID HTML moves from +2.1% in the primary cohort to −0.8%, so its initial cost is not repeatable. No source or binary changes intervene. The [raw repeat](repeat.json.gz) retains 294 further process samples; [its summary](repeat-summary.json) is separate.

| Operation / workload | Time change | Spread B / C |
| --- | ---: | ---: |
| html / readme | -0.10% | 1.7% / 2.1% |
| ast-html / readme | -0.16% | 2.3% / 2.8% |
| markdown / readme | -0.05% | 1.9% / 1.6% |
| html / mixed-100k | -0.56% | 1.5% / 2.1% |
| ast-html / mixed-100k | +0.70% | 2.5% / 4.3% |
| markdown / mixed-100k | -1.29% | 1.8% / 2.6% |
| html / mixed-1m | +0.39% | 3.6% / 5.7% |
| ast-html / mixed-1m | +1.02% | 2.9% / 2.6% |
| markdown / mixed-1m | -0.14% | 1.7% / 22.6% |
| html / gfm-100k | +0.04% | 2.7% / 1.0% |
| ast-html / gfm-100k | +0.24% | 14.1% / 2.0% |
| markdown / gfm-100k | -0.35% | 2.0% / 2.5% |
| html / rust-nll-rfc | -1.10% | 2.7% / 2.8% |
| ast-html / rust-nll-rfc | -0.66% | 3.1% / 1.7% |
| markdown / rust-nll-rfc | -1.07% | 2.7% / 3.6% |
| html / titles-and-definitions | -1.43% | 1.8% / 1.6% |
| ast-html / titles-and-definitions | -0.42% | 2.4% / 5.9% |
| markdown / titles-and-definitions | +1.33% | 2.5% / 3.0% |
| html / headings | -0.76% | 4.4% / 2.5% |
| ast-html / headings | -0.94% | 3.1% / 2.7% |
| markdown / headings | -7.66% | 2.3% / 3.4% |


## Correctness and allocations

The [preservation report](preservation.json) compares all 20 workloads: direct and AST HTML, Markdown, heading values, and parser/document events with resolved values and authored byte spans. All agree. Semantic text may deliberately change storage from owned strings to source ranges, so storage variants are normalized for this comparison. Diagnostic hashes are retained; complete diagnostics can be regenerated with the driver. Default-feature workspace tests and all-feature tests retain the repository's full construct corpora.

The [allocation comparison](allocation-summary.json) retains every warmed operation. Examples:

- 2,048 ordinary titled images avoid 2,048 allocations in both direct and prepared HTML.
- Leading-escaped link/reference metadata avoids 8,192 allocations in complete HTML conversion.
- Heading serialization avoids 2,048 allocations and 3,072 reallocations in the ordinary and forced-setext cases; the multiline case avoids 4,096 reallocations.
- The original mixed 1 MB sample avoids 2,567 allocations in prepared Markdown serialization.
- The pipe-heavy table retains its allocation/reallocation counts, with three additional requested bytes from different scratch-buffer growth. Table cells without pipes avoid one allocation and one reallocation.

Requested bytes measure cumulative allocation traffic, including full reallocation requests, rather than peak or live memory. [Raw counts](allocations.json.gz) come from separate instrumented executables; timing uses the system allocator.

Formatting, all-target/all-feature workspace Clippy, native coverage and workspace tests pass. Coverage is 14,072/14,072 measured source regions, with 21 invariant-only unreachable exclusions. [Validation](validation.json) records commands and log hashes. Linux runtime coverage is checked by CI. Independent adversarial review covers source semantics and benchmark methodology.

## Reproduction

Run these scripts from this study directory. Apple M4 Pro, AArch64 macOS. [Implementation metadata](implementation-metadata.json) records revisions, compiler, source-tree identities, driver/lockfile hashes and frozen binary hashes. Seven sequential alternating process samples per engine and workload; every process warms for 100 ms, then runs a calibrated approximately 120 ms batch with the same iteration count for both engines. Both run through the same launch path. No project builds, tests or diagnostic runs overlap timing. The primary and repeat cohorts are kept separate; no samples are discarded or pooled.

Direct HTML includes parsing, rendering and disposal. Prepared HTML and Markdown exclude document construction and include fresh output allocation and disposal. Timing enables the crate's default features. Diagnostic and allocation binaries are separate feature builds. HTML uses the original comparison's permissive HTML/protocol settings; the GFM preset disables footnotes for continuity with those examples. Frozen input bytes are included in compressed form.

```sh
python3 build.py --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/deferred-study
python3 run.py --binary baseline=/tmp/deferred-study/baseline-bench --binary candidate=/tmp/deferred-study/candidate-bench --data /tmp/deferred-study/data/original.json --data /tmp/deferred-study/data/real.json --data /tmp/deferred-study/data/focused.json --out /tmp/deferred-study/primary.json
python3 run.py --binary baseline=/tmp/deferred-study/baseline-diagnostics --binary candidate=/tmp/deferred-study/candidate-diagnostics --check --data /tmp/deferred-study/data/original.json --data /tmp/deferred-study/data/real.json --data /tmp/deferred-study/data/focused.json --out /tmp/deferred-study/preservation.json
python3 run.py --binary baseline=/tmp/deferred-study/baseline-allocations --binary candidate=/tmp/deferred-study/candidate-allocations --alloc --data /tmp/deferred-study/data/original.json --data /tmp/deferred-study/data/real.json --data /tmp/deferred-study/data/focused.json --out /tmp/deferred-study/allocations.json
```
