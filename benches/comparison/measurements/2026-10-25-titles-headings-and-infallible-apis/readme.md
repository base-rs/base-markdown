# Shared title decoding, heading semantics and infallible APIs

This study compares merged main `d65d7210b62210a3a855080516b55a83bea992cf` with implementation `1b7b1b0e3a7a99d065c3cb059e13bd54e9d2f1e3`. The baseline was built from PR #62 head `4a60bfeaacef439833a42610638aadfcabf45ee8`; its complete Git tree matches that squash merge. Three implementation commits fuse title normalization/decoding, share heading text and terminal-ending rules, and remove obsolete error wrappers.

`EventParser::new` returns its parser directly; `to_html_with_options` returns `String`. This is an intentional breaking API change. Remove `?`, `.unwrap()` or `.expect(...)` at those two calls. Shared event consumers, document validation/editing, custom stream rendering and Markdown serialization retain their meaningful errors. See the [migration](../../../../context/pull-parser.md#infallible-construction-and-html-conversion).

## Implementation

A single `ValueBuilder` now removes physical continuation indentation and decodes inline titles. The shared byte scanner skips ordinary runs; unchanged contiguous values remain source-backed. Entity-produced newlines do not strip following spaces, independently disabled escape/entity options remain respected, and inline HTML uses the same construction without decoding. Numeric references use a four-byte stack buffer rather than a temporary `String`. Existing logical-to-source mapping still applies after construction.

The AST and HTML heading-ID paths share semantic text contributions, with block-code contributions retained only for general AST text. Heading enumeration, IDs and Markdown serialization share removal of exactly one CRLF, CR or LF. No parser state, dependency or per-event allocation is added.

## Performance

The 256-title workloads take 13.6% less time with escapes/entities and 24.3% less with numeric entities; ordinary multiline titles take 2.9% less. Prepared-document Markdown serialization remains within 1.1% of baseline. Ordinary HTML shows a small cost: 1.2–2.5% more time in the primary cohort. A separate repeat and intermediate-commit comparisons are retained below rather than treating the cost as noise. Inkmark was not retimed.

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.012531 | 0.012777 | +1.96% | 1.4% / 1.2% |
| html/mixed-100k | 0.377403 | 0.383770 | +1.69% | 2.0% / 1.0% |
| html/mixed-1m | 3.786014 | 3.879108 | +2.46% | 2.1% / 0.5% |
| html/gfm-100k | 0.585836 | 0.592861 | +1.20% | 1.9% / 3.6% |
| markdown/readme | 0.060942 | 0.060442 | -0.82% | 1.0% / 2.1% |
| markdown/mixed-100k | 1.352217 | 1.351299 | -0.07% | 1.4% / 0.6% |
| markdown/mixed-1m | 13.548320 | 13.499285 | -0.36% | 2.0% / 0.1% |
| markdown/gfm-100k | 1.560900 | 1.545080 | -1.01% | 0.6% / 0.8% |

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| features/title-single-line-1 | 0.000374 | 0.000366 | -2.02% | 1.7% / 2.6% |
| features/title-single-line-256 | 0.042051 | 0.039766 | -5.43% | 0.4% / 0.4% |
| features/title-multiline-1 | 0.000529 | 0.000524 | -0.92% | 3.1% / 2.3% |
| features/title-multiline-256 | 0.066613 | 0.064668 | -2.92% | 0.8% / 1.9% |
| features/title-decoded-multiline-1 | 0.000679 | 0.000620 | -8.61% | 3.5% / 1.7% |
| features/title-decoded-multiline-256 | 0.116795 | 0.100943 | -13.57% | 2.2% / 0.3% |
| features/title-numeric-multiline-1 | 0.000618 | 0.000520 | -15.81% | 2.6% / 1.7% |
| features/title-numeric-multiline-256 | 0.102768 | 0.077807 | -24.29% | 1.2% / 0.1% |
| features/headings-html-256 | 0.192816 | 0.196830 | +2.08% | 0.5% / 1.4% |
| features/headings-headings-256 | 0.095027 | 0.095545 | +0.55% | 3.1% / 2.8% |
| features/inline-html-multiline-256 | 0.075828 | 0.075191 | -0.84% | 1.3% / 1.7% |

Changes are `100 × (candidate / baseline − 1)`; negative means less time. Spread is `(maximum process median / minimum process median − 1) × 100`, not a confidence interval. The [primary raw samples](primary.jsonl) contain 18 process/mode records, 114 document results and 798 batch samples; the [summary](primary-summary.json) retains every process median.

## HTML follow-up

The ordinary-HTML repeat uses the same frozen binaries with no intervening source changes. It confirms a small cost. Its six process/mode records, 24 document results and 168 samples are retained separately in [raw results](html-repeat.jsonl) and [summary](html-repeat-summary.json).

| Workload | Baseline ms | Candidate ms | Time change | Spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.012360 | 0.012536 | +1.43% | 2.3% / 1.0% |
| html/mixed-100k | 0.371548 | 0.379996 | +2.27% | 1.7% / 1.1% |
| html/mixed-1m | 3.759109 | 3.828257 | +1.84% | 0.5% / 0.6% |
| html/gfm-100k | 0.580048 | 0.587402 | +1.27% | 1.0% / 0.8% |

To isolate the implementation stages, another cohort compares the baseline with title commit `bf68305`, heading commit `fcbb4b0`, and final implementation `1b7b1b0`. All stages use the same adapter and old APIs until the final migration. Results below are relative to that cohort's baseline; they do not identify one consistently dominant cause. The small cost is retained as a tradeoff for consolidation and simpler public APIs, alongside the larger focused title gains. There are no layout padding tricks, caches or extra fast paths added to recover it.

| HTML workload | Titles | + Shared headings | + Infallible APIs |
| --- | ---: | ---: | ---: |
| html/readme | +1.30% | +1.98% | +1.98% |
| html/mixed-100k | +0.70% | +0.75% | +1.42% |
| html/mixed-1m | +0.62% | +0.47% | +0.11% |
| html/gfm-100k | +0.61% | +1.51% | +2.86% |

The [stage raw samples](stages.jsonl) and [summary](stages-summary.json) preserve 12 process/mode records, 48 document results and 336 samples. No samples were discarded or pooled across cohorts.

## Output and allocation checks

The [established preservation report](preservation.json) compares 8,468 corpus outputs, 58,596 probes, 120 complete edited arenas, the original direct/checked/document HTML and Markdown outputs, and 23,449 additional serializer/footnote records. [Focused checks](focused-preservation.json) cover 11 title and heading workloads, including complete heading entries and syntax metadata. All compared outputs agree byte for byte.

[Focused allocation counts](focused-allocations.json) show 256 fewer allocations and 256 fewer reallocations for 256 decoded multiline titles. Numeric titles avoid 1,280 allocations and 256 reallocations. Requested bytes fall by 13,312 and 15,872 respectively. Other focused counts and the original examples' counts are unchanged. [Established counts](allocations.json) retain 63 further records. Counts cover one warmed operation; requested bytes are cumulative allocation/growth requests, not peak memory. Instrumented executables are separate from native-allocator timing executables.

Native coverage passes 13,828/13,828 measured regions with 21 invariant-only unreachable exclusions. Workspace tests, all-feature/all-target Clippy and formatting pass. [Validation](validation.json) records source identities, commands and log hashes; Linux runtime coverage remains the CI gate. Independent source review found and resolved stale configuration doctests after the API migration and confirmed the final implementation.

## Method and reproduction

Apple M4 Pro, AArch64 macOS, Rust 1.99.1/LLVM 23.1.3. [Metadata](metadata.json) pins revisions, binary hashes, compiler, adapters, lockfiles and inputs. Three sequential alternating rounds per cohort reverse engine order in round two. Each workload warms for 200 ms and runs seven calibrated approximately 150 ms batches. Tables use the median of three process medians. HTML includes parsing, rendering and output disposal. Markdown and heading enumeration operate on prepared documents; setup and preparation are excluded. Own builds, tests and output/allocation diagnostics finish before each cohort. Lightweight source/log inspection may overlap the primary cohort; the HTML repeat and stage cohort have no own review or diagnostic overlap. The shared host has no affinity or isolation.

The new build driver reuses historical adapters unchanged for baseline/stages and migrates only staged candidate copies to the new API. Historical committed sources keep their original signatures and remain reproducible. Staged source hashes are recorded in metadata. Existing ignored corpus fixtures must be prepared as described in the preceding studies.

```sh
study=benches/comparison/measurements/2026-10-25-titles-headings-and-infallible-apis
python3 "$study/prepare.py"
python3 "$study/build.py" --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-text-delimiters-and-footnotes/verify.py --work /tmp/study
python3 "$study/verify.py" --work /tmp/study
python3 "$study/measure.py" --work /tmp/study --output /tmp/study/primary.jsonl --mode html --mode markdown --mode features
python3 "$study/measure.py" --work /tmp/study --output /tmp/study/html-repeat.jsonl --mode html
python3 "$study/build.py" --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study \
  --stage-only --stage titles=/path/to/bf68305 --stage headings=/path/to/fcbb4b0
python3 "$study/measure.py" --work /tmp/study --output /tmp/study/stages.jsonl --mode html \
  --engine baseline --engine titles --engine headings --engine candidate
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/primary.jsonl --input /tmp/study/html-repeat.jsonl --input /tmp/study/stages.jsonl
```

Corpus attribution remains in the preceding studies. Generated fixtures are ignored and hashed. Frozen executables remain outside Cargo target directories.
