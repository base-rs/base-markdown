# Markdown parser comparison

This harness compares the public Markdown-to-HTML APIs of core-markdown, markdown-rs, Comrak, Markly, Commonmarker, and Inkmark. It lives outside the main workspace so competitor dependencies do not affect the published crate or ordinary tests. Rust dependencies and Ruby gems are pinned in the adjacent lockfiles.

## Reproduction

Use a release Rust toolchain, Python 3, Ruby, Bundler, and a compiler for Markly's native extension:

```sh
cd benches/comparison
bundle config set --local path vendor/bundle
bundle install
python3 run.py
python3 check.py --all
```

`run.py` runs all six engines sequentially and saves raw batch timings in `results/`. Supply parser names to time a subset, for example `python3 run.py core-markdown inkmark`. `check.py` checks direct/arena rendering parity and exact core-markdown HTML against all 652 CommonMark 0.31.2 reference examples and the pinned markdown-rs version on all 677 published GFM examples, plus 29 targeted inputs under four profiles. `--all` additionally records the six engines' agreement counts and complete outputs in `results/`. Downloaded specification sources are checked against recorded SHA-256 hashes; an unexpected source change requires inspection. The CommonMark and GFM specifications are attributed to their respective authors and licensed CC BY-SA 4.0; downloaded copies remain under ignored `data/`.

Individual Rust adapters support `bench`, `render`, and a 30-second `profile` loop:

```sh
target/release/markdown-comparison render core-markdown commonmark data/commonmark.json
target/release/markdown-comparison profile core-markdown commonmark data/bench-commonmark.json
bundle exec ruby adapter.rb inkmark render commonmark data/commonmark.json
```

The JSON input is an array of objects containing `markdown` and, for timings, `name`. `core-prepare` times parser construction and disposal; `core-events` additionally drains resolved events into `black_box`, without rendering. `core-arena` measures parsing into the source-backed document and rendering its event traversal. `core-ast` measures the separate legacy mdast/HTML-renderer path. These diagnostic modes are separate from the direct-HTML path in the six-engine table. `prepare.py` also generates `bench-scanners.json` (long prose and fenced code), `bench-references.json`, `bench-definitions.json`, and `bench-linked-attention.json` for focused diagnostics. The definition workload has 256, 1,024, and 4,096 unique labels in reverse authored order, 25% additional case-equivalent duplicates, and two forward references.

Allocation counting uses a separate executable so timing runs retain the system allocator:

```sh
cargo build --release --locked
target/release/allocations commonmark data/bench-commonmark.json
target/release/allocations gfm data/bench-gfm.json
python3 run.py core-arena
```

Counts exclude input loading and options construction. They report allocation calls, reallocations, and cumulative requested bytes, including reallocation requests; requested bytes are not peak or retained memory. Parser preparation, pulling after preparation, complete arena construction, traversal, cold HTML, HTML with reserved output, and legacy mdast construction are recorded separately. Timing and allocation profiles use the same extension and HTML options.

## Method

Each document receives at least 200 ms of warmup, followed by seven batches calibrated to approximately 150 ms each. The result is the median batch time per complete conversion. Process startup, library loading, input I/O, JSON transport, and initial options construction are excluded. Parsing, HTML generation, result disposal, and ordinary Ruby garbage collection are included. No rendered output or parsed document is cached. Ruby calls retain the public APIs' internal option handling. Runs are sequential, without CPU affinity; small differences should not be interpreted as universal rankings.

The CommonMark profile disables extensions, allows raw HTML and all URL protocols, preserves soft breaks, and disables syntax highlighting and other presentation additions. The GFM profile enables tables, strikethrough, tasks, and literal autolinks, with footnotes disabled. These permissive configurations match the specification fixtures and are chosen for comparison.

The saved README is 9,019 bytes. Deterministic mixed documents of 100,252 and 1,000,020 bytes repeat sections containing prose, emphasis, links, inline code, Unicode, entities, block quotes, nested lists, and fenced Rust code. The 100,130-byte GFM document additionally includes tables, tasks, and autolinks. `prepare.py` regenerates the workloads from the saved README and source literals.

**GFM is not an equal-output throughput comparison:** Inkmark leaves `www.example.com` unlinked in this workload even with autolinking enabled. Its table alignment attributes and empty table bodies also differ. Comrak and Commonmarker do not expose the same independent GFM tag filter as the other adapters. Read GFM timings alongside the output agreement results.

## Source-backed text coalescing follow-up

The compact scanner combines escaped punctuation with following ordinary source text, preserving source ranges and existing structured link/code details. On mixed 1 MB this removes 5,134 semantic events (3.0%) and AST nodes (4.1%), with unchanged allocation measurements and output bytes. Three alternating runs compare merged main `3f633ab` with `7d853c7`:

| Workload | Merged main (ms) | Candidate (ms) | Inkmark 0.2.1 (ms) |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0150 | 0.0147 | 0.0187 |
| Mixed 100 KB | 0.4418 | 0.4368 | 0.4088 |
| Mixed 1 MB | 4.4506 | 4.4128 | 4.3402 |
| GFM 100 KB | 0.6477 | 0.6488 | 0.7082 |

Mixed HTML takes 1.1% and 0.8% less time, close to ordinary measurement variation. The separate public event mode takes 3.0% and 1.8% less, and AST parsing/rendering takes 2.9% and 2.1% less. Mixed HTML remains 6.9% and 1.7% slower than Inkmark. GFM retains the output differences described above. [Full measurements](measurements/2026-10-24-text-coalescing/readme.md) retain every sample, event/node counts, allocations, output checks, and review evidence. Ruby is 4.0.9 in this run, versus 4.0.7 below; compare engines within each recorded run.

## Fixed HTML write follow-up

The shared renderer now batches fixed tags and attribute delimiters. On the mixed 1 MB fixture, this reduces logical writer calls by 10.5%, with unchanged output and allocation counts. Three alternating runs on Apple M4 Pro compare merged main `3750a52` with production `aa2c98c` and focused tests `01a1a07`:

| Workload | Merged main (ms) | Candidate (ms) | Inkmark 0.2.1 (ms) |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0150 | 0.0149 | 0.0184 |
| Mixed 100 KB | 0.4402 | 0.4313 | 0.3987 |
| Mixed 1 MB | 4.4696 | 4.4054 | 4.2833 |
| GFM 100 KB | 0.6444 | 0.6420 | 0.6954 |
| References | 0.6478 | 0.6143 | 0.8418 |

Mixed rendering takes 2.0% and 1.4% less time, and references 5.2% less. The mixed workloads remain 8.2% and 2.9% slower than Inkmark. Small differences remain sensitive to compiler/process layout; exploratory mixed results were closer to neutral. Long prose and fenced code stay close to baseline. GFM retains the output differences described above. [Full measurements and evidence](measurements/2026-10-24-html-writes/readme.md) include every sample, allocation counts, output checks, review, and rejected validation/escaping experiments.

## Event delivery and buffer follow-up

This follow-up compares merged main `dc4f5ee` with production source `8d38420`. Literal-link prefix tags reduce private inline records from 56 to 48 bytes on this compiler. Closing spans are computed once with inlined lookup, and direct HTML starts with a reservation capped at 4 KiB. The public event contract is unchanged.

The values below are milliseconds, using three alternating main/candidate/Inkmark process runs on Apple M4 Pro. Both Rust revisions use the same launch path, arguments, and environment. Builds, tests, and independent review finished before timing. [All samples, hashes, allocations, output checks, rejected experiments, and review evidence](measurements/2026-10-24-commonmark-events/readme.md) are retained.

| Workload | Merged main | Candidate | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0155 | 0.0149 | 0.0186 |
| Mixed 100 KB | 0.4401 | 0.4371 | 0.4048 |
| Mixed 1 MB | 4.4920 | 4.4437 | 4.2797 |
| GFM 100 KB | 0.6578 | 0.6512 | 0.6998 |
| References | 0.6465 | 0.6488 | 0.8430 |

HTML improvements are modest: README takes 4.0% less time, mixed 100 KB 0.7%, and mixed 1 MB 1.1%. References change by +0.4%, long prose by -0.5%, and code by +0.1%; these small changes are effectively neutral. Mixed 100 KB and 1 MB remain 8.0% and 3.8% slower than Inkmark. The overall-fastest target is not met.

Preparing and draining the public event iterator falls from 3.785 to 3.528 ms on mixed 1 MB (6.8% less time), and from 0.559 to 0.511 ms on GFM (8.4% less time). These modes include preparation and event disposal but no rendering; they are separate compiled loops and must not be subtracted from HTML timings to infer renderer cost.

A full-source output reservation regressed long prose by about 9% in repeated isolation checks and was replaced with the 4 KiB cap. The final three-round scanner comparison is effectively neutral. The cap limits initial speculative reservation, not final output capacity. Documents rendering to no HTML can still retain up to 4 KiB. All 2,117 saved outputs remain identical across direct, checked-event, and arena rendering. The existing GFM output differences from Inkmark remain; its timing lead is not an equal-output ranking.

## Compact HTML and GFM follow-up

The biggest remaining costs were compatibility fallbacks. Two HTML comments forced the entire saved README onto grammar block preparation. GFM options forced both block preparation and ordinary inline regions through grammar tokens, even when the text itself used only CommonMark syntax. The initial GFM diagnostic spent 2.89 ms preparing blocks and made 28,525 allocation calls per HTML conversion. Removing the README comments as a diagnostic reduced conversion from 0.065 to 0.015 ms; the comment text itself was not the expensive part.

Common HTML blocks now use compact source fragments. Ordinary GFM blocks and tables use compact records, and ordinary inlines, tasks, strikethrough, and simple literal links share reusable inline candidates. Complex boundary cases retain grammar fallback. Compact extension dispatch is cached once, and CommonMark delimiter searches exclude the strikethrough bucket. No public feature was removed and no runtime dependency was added.

The final comparison uses merged main `164155bc2e24b9d2ad5ec1f219e989a6e4df49d6` and production source `5a096de7da9ace6da062a391c225e1653f485c87`. The original four inputs are unchanged, verified by their saved hashes. [Raw samples, revisions, allocation counts, output checks, and review evidence](measurements/2026-10-24-html-gfm/) are retained, including exploratory results, the earlier complete snapshot, and dispatch-tuning comparisons. Final timings are medians of three process medians from alternating main/candidate/Inkmark runs on Apple M4 Pro, with the same launch path, arguments, and environment for both Rust revisions. Project builds, tests, and review workloads finished before timing; normal machine background activity was not isolated. All times below are milliseconds.

| Workload | Merged main | This change | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0648 | 0.0151 | 0.0181 |
| Mixed 100 KB | 0.4129 | 0.4310 | 0.3927 |
| Mixed 1 MB | 4.1682 | 4.3493 | 4.1533 |
| GFM 100 KB | 4.9825 | 0.6415 | 0.6857 |

README is **4.28× faster than main** and takes **16.2% less time than Inkmark**. GFM is **7.77× faster than main** and takes **6.5% less time than Inkmark**. Candidate GFM process medians span 0.639–0.642 ms, versus Inkmark's 0.683–0.688 ms.

**The overall fastest-parser target remains unmet.** Mixed 100 KB and 1 MB rendering take 4.4% and 4.3% more time than main, and remain slower than Inkmark. Reference rendering is 0.635 ms versus main's 0.625 ms and Inkmark's 0.838 ms. No ordinary CommonMark speedup is claimed. The remaining work includes compact dispatch and event delivery; the process-layout sensitivity demonstrated in earlier studies also limits conclusions from small differences.

All benchmark outputs are byte-identical to main. The three original CommonMark workloads agree with Inkmark after entity decoding, but are not byte-identical. **GFM still produces different output:** core-markdown links `www.example.com`, while Inkmark leaves it literal, and table serialization differs. The timing lead therefore does not establish an equal-output GFM ranking. [Output agreement](measurements/2026-10-24-html-gfm/benchmark-output-agreement.json) records the comparisons and hashes.

Allocation calls / reallocations for complete HTML conversion are:

| Workload | Merged main | This change |
| --- | ---: | ---: |
| README | 247 / 91 | 17 / 22 |
| GFM 100 KB | 28,525 / 3,401 | 372 / 388 |

GFM block preparation falls from 13,833 allocation calls and 874 reallocations to **4 and 11**. Pulling after preparation falls from 14,690 calls and 2,511 reallocations to **366 and 361**. Constructed `http://` and `mailto:` destinations still allocate. Arena traversal remains allocation-free. Ordinary mixed-document allocation counts are unchanged. GFM emits 24,525 events versus 23,810 previously because compact text fragments have different segmentation; rendering remains identical.

Single-process phase diagnostics include construction, draining where applicable, and disposal; these are complete operations, not additive phases:

| Workload | Main preparation | Candidate preparation | Main preparation + events | Candidate preparation + events |
| --- | ---: | ---: | ---: | ---: |
| README | 0.0503 | 0.0037 | 0.0595 | 0.0111 |
| GFM 100 KB | 2.9122 | 0.1548 | 4.7745 | 0.5560 |
| Mixed 1 MB | 0.8971 | 0.9424 | 3.1161 | 3.7697 |

The ordinary mixed event-draining diagnostic also regresses and needs separate optimization. Definition and long-prose/code regression observations are saved in `summary.json`; they are single-process medians and are not new cross-engine ranking claims.

The final source preserves **all 2,117 saved outputs** across direct HTML, checked events, and arena rendering. Fresh six-engine consistency counts are unchanged. Independent adversarial review covered **271,902 source/option combinations**, with zero panics, rendering-path disagreements, or checked metadata/span differences from forced grammar. Review found and fixed mixed delimiter precedence, unclosed HTML line ownership, lazy table headers, and EOF HTML provenance. Outside the saved corpus, GFM inherits the documented compact CommonMark corrections for list tightness, container tabs, code endings, emphasis, and incomplete fences in lazy paragraphs. Unusual legacy table dispatch remains preserved. See the [architecture](../../context/pull-parser.md) and [review evidence](measurements/2026-10-24-html-gfm/independent-review.json) for exact boundaries and remaining reference-engine differences.

Final local arm64 all-feature coverage passes **26,897/26,897 measured regions**, excluding 68 invariant-only unreachable regions. The portable x86_64 path cross-compiles without default features. These checks establish measured coverage and observed compatibility, not proof over every input.

## Compact definition-preparation follow-up (previous study)

Ordinary same-line reference definitions now stay in compact block storage. The parser keeps source-backed labels, destinations, and titles in authored order, normalizes each label once, and sorts a separate index for first-definition-wins lookup. Duplicate definition events remain available to AST consumers. Both inline producers share the table; grammar inline fallback materializes normalized keys only when it is actually needed. Forward references still resolve before event delivery. Multiline definitions, ambiguous continuations, and container-tab provenance that needs the grammar retain compatibility preparation.

This study compares merged baseline `8463e25128a5f1459793791bd0cd35cc3bb3068e` with production source `a863c3a32c24e07fd11e0618f0cfc242f0635db6`; the new definition fixture generator is `363dc83e56fda10580166503a8adda5295c15399`. [Raw timings, allocation counts, output checks, hashes, and review results](measurements/2026-10-23-definitions/) are retained. The machine and engine versions are the same as in the earlier studies below. These values are medians of three process medians from alternating baseline/candidate/Inkmark sequences, in milliseconds:

| Engine / source | References 80 KB | README 9 KB | Mixed 100 KB | Mixed 1 MB |
| --- | ---: | ---: | ---: | ---: |
| core-markdown, merged baseline | 1.078 | 0.068 | 0.428 | 4.290 |
| core-markdown, compact definitions | 0.632 | 0.068 | 0.434 | 4.383 |
| Inkmark 0.2.1 | 0.851 | 0.019 | 0.408 | 4.400 |

Reference conversion is **1.71× faster than the merged baseline** and takes **25.8% less time than Inkmark**, with byte-identical HTML. Candidate process medians span 0.632–0.633 ms versus Inkmark's 0.850–0.851 ms. The reference target is met on this workload. **The overall Inkmark target remains unmet:** README remains substantially slower, the largest definition-heavy case below trails Inkmark, and GFM retains grammar preparation. This change does not remove public features or add a runtime dependency.

README latency is unchanged. Mixed 100 KB and 1 MB are respectively 1.5% and 2.2% slower than the baseline in these runs; no mixed-document speedup is claimed. Rust comparisons use the same executable path, arguments, and environment. An additional alternating environment-padding sweep over eight lengths records mixed 1 MB ranges of 4.246–5.676 ms for the baseline and 4.347–4.441 ms for the candidate, with medians of 4.311 and 4.381 ms. The small slowdown remains in the sweep medians, while the wider baseline range also demonstrates the process-layout sensitivity found in the earlier study. Every sample is retained, including exploratory measurements before correctness fixes; those initial measurements are excluded from these final claims. Builds, tests, and independent review workloads finished before timing; normal machine background activity was not isolated.

Reference allocation counts are:

| Phase | Baseline allocation calls / reallocations | Candidate allocation calls / reallocations |
| --- | ---: | ---: |
| Block preparation | 2,062 / 119 | 5 / 2 |
| Pulling after preparation | 4 / 13 | 3 / 13 |
| Complete HTML conversion | 2,068 / 148 | 10 / 31 |
| Arena traversal | 0 / 0 | 0 / 0 |

Preparation falls from 0.447 to 0.024 ms; preparation plus event draining falls from 0.822 to 0.453 ms. These are independent complete operations including disposal, not additive phases. Both sources emit 26,005 events. Mixed-document allocation counts are unchanged; README adds one preparation allocation (152 requested bytes) for the shared definition index. Growing buffers and normalized label strings still allocate, so this is not an allocation-free parser.

The new scaling workload authors 256, 1,024, or 4,096 unique definitions in reverse order, followed by 25% case-equivalent duplicates, with two forward references. Each cell below is one process median of seven calibrated batches, so small cross-engine differences need repeated confirmation:

| Unique definitions | Merged baseline | Compact definitions | Inkmark |
| --- | ---: | ---: | ---: |
| 256 | 0.462 | 0.041 | 0.043 |
| 1,024 | 2.501 | 0.169 | 0.169 |
| 4,096 | 19.851 | 0.741 | 0.708 |

All three workloads produce byte-identical core/Inkmark HTML and preserve baseline output. At 4,096 unique definitions, conversion is **26.8× faster than the baseline** but still about 4.7% slower than Inkmark. Allocation calls fall from 58,425 to 5,129 and reallocations from 197 to 20. The remaining calls mostly normalize authored labels; this work removes repeated linear duplicate searches, not every per-definition allocation.

The final source preserves all **2,117 saved outputs** across direct HTML, checked events, and arena rendering. Fresh six-engine consistency counts match the table below. Independent review exercised **36,446 sources** with no panics or rendering-path mismatches and identical definition values/spans to forced grammar preparation. Both GFM profiles preserve all reviewed baseline outputs. Outside the saved corpus, 371 CommonMark outputs change: 228 now agree with normalized Comrak, 116 differ only in formatting, and 27 improve indentation or list tightness while retaining other pre-existing discrepancies. No reviewed case regresses from matching normalized Comrak to disagreeing. These compact-container corrections and conservative fallback boundaries are documented in the [architecture](../../context/pull-parser.md). Local arm64 all-feature coverage passes **25,925/25,925 measured regions**, with 68 invariant-only unreachable regions excluded; the portable x86_64 path cross-compiles without default features.

## Reference-resolution follow-up (previous study)

This earlier optimization keeps same-line full, collapsed, and shortcut references in reusable compact inline storage. It reuses ASCII label normalization, binary-searches a sorted definition table, and stores definition indices instead of per-reference metadata. The first authored definition still wins. Ordinary destinations and titles remain source ranges; Unicode normalization and constructed values still allocate. Definition-free compact blocks specialize preparation so reference lookup and label bookkeeping compile out.

This study compares merged baseline `6b64e9baaec8e6e5c4a64eef92b76f42a84f801c` with production source `b67d71727d2aabd8e73a6b55f96930e603948a75`, using the unchanged harness on the machine described below. [All samples, revisions, input hashes, output agreement, and validation records](measurements/2026-10-23-references/) are retained. Values below are medians of three process medians from alternating baseline/candidate/Inkmark sequences, in milliseconds:

| Engine / source | References 80 KB | README 9 KB | Mixed 100 KB | Mixed 1 MB |
| --- | ---: | ---: | ---: | ---: |
| core-markdown, merged baseline | 5.058 | 0.088 | 0.415 | 4.174 |
| core-markdown, compact references | 1.056 | 0.066 | 0.419 | 4.193 |
| Inkmark 0.2.1 | 0.843 | 0.018 | 0.405 | 4.275 |

References improve **4.8×** and README latency falls **25%**. Mixed-document differences between core sources are within 1% in this final comparison; no mixed-document speedup is claimed. References produce byte-identical HTML across core-markdown and Inkmark. README and mixed outputs differ only in equivalent text-quote escaping. **The overall Inkmark target remains unmet:** the remaining reference, README, and GFM gaps are substantial; the small mixed-workload difference is not a reliable engine ranking.

The timing investigation exposed process-layout sensitivity. Initial paired runs suggested a 10% mixed-document regression, but changing only environment variables moved unchanged binaries by 10–20% and could reverse their ranking. Final Rust comparisons use an identical executable path, arguments, and environment. An additional alternating sweep varies one environment-padding value over eight lengths: mixed 1 MB spans 4.182–5.377 ms for the baseline and 4.234–5.380 ms for the candidate, with medians of 4.295 and 4.275 ms respectively. This does not establish the hardware mechanism, but prevents attributing those shifts solely to parser changes. Initial runs and every diagnostic are retained alongside the final results; builds, tests, and independent review workloads do not overlap timings.

Reference-fixture allocations show the more stable improvement:

| Phase | Baseline allocation calls / reallocations | Candidate allocation calls / reallocations |
| --- | ---: | ---: |
| Block preparation | 2,062 / 119 | 2,062 / 119 |
| Pulling after preparation | 58,023 / 2,086 | 4 / 13 |
| Complete HTML conversion | 60,087 / 2,221 | 2,068 / 148 |
| Arena traversal | 0 / 0 | 0 / 0 |

Preparation still uses grammar blocks and accounts for most remaining allocation calls. Candidate preparation takes 0.436 ms; preparation plus event draining takes 0.820 ms. These are separate complete operations including disposal, not additive phase timings. Growing buffers remain necessary; this is not a strictly allocation-free parser.

The final source preserves all 2,117 saved outputs across direct HTML, checked events, and arena rendering. Fresh six-engine consistency counts are unchanged from the table below. Expanded-tab label limits, malformed suffix precedence, duplicate definitions, Unicode, and nested links/images have regression coverage. Outside the saved corpus, 52 reviewed emphasis corrections agree with Comrak; for example, `[a]*a**_` with `[a]: /a` now emphasizes `a` after the link. Local arm64 coverage passes **25,621/25,621 measured regions**, excluding 68 invariant-only unreachable regions; the portable x86_64 path cross-compiles without default features. No public feature or runtime dependency changes in this follow-up.

## Compact-parser measurements (merged baseline)

Measured on 2026-10-23 using an Apple M4 Pro, macOS 27.0.1 arm64, Rust 1.99.1, and Ruby 4.0.7 with YJIT disabled. The Rust engines use release builds without syntax highlighting; Commonmarker and Inkmark use published arm64-darwin binaries, and Markly is built locally. Commonmarker embeds the Comrak 0.56 series, while the standalone adapter uses Comrak 0.57.0.

The final production source is `6162ee825db3435ab7ff2dc139a73faa63e4e8f7`; the latest harness change is `c62db79f7cd09c5b8b76bd3aca4664bfbc3956e2`. [Raw samples, source revisions, and input hashes](measurements/2026-10-23-compact/) are committed together. Builds, test suites, and independent review workloads finished before these sequential timing runs. Normal machine background activity remains; no CPU affinity was set.

Times are **milliseconds per conversion; lower is faster**.

| Engine | README 9 KB | Mixed 100 KB | Mixed 1 MB | GFM 100 KB |
| --- | ---: | ---: | ---: | ---: |
| core-markdown (compact pull parser) | 0.088 | 0.494 | 4.968 | 5.049 |
| markdown-rs 1.0.0 | 0.368 | 13.636 | 548.437 | 22.904 |
| Comrak 0.57.0 | 0.030 | 0.909 | 8.970 | 1.343 |
| Markly 0.19.0 | 0.040 | 1.007 | 11.808 | 1.982 |
| Commonmarker 2.10.1 | 0.043 | 1.088 | 10.684 | 1.559 |
| Inkmark 0.2.1 | 0.018 | 0.395 | 4.199 | 0.697 |

The table is the second complete six-engine snapshot. The first snapshot and both sets of three alternating core/Inkmark pairs are retained:

| Run / pair | Order | core-markdown mixed 1 MB | Inkmark mixed 1 MB |
| --- | --- | ---: | ---: |
| first / 1 | core → Inkmark | 5.368 | 4.304 |
| first / 2 | Inkmark → core | 5.062 | 4.247 |
| first / 3 | core → Inkmark | 5.174 | 4.459 |
| final / 1 | core → Inkmark | 5.017 | 4.203 |
| final / 2 | Inkmark → core | 5.047 | 4.242 |
| final / 3 | core → Inkmark | 5.034 | 4.216 |

The compact parser substantially narrows the mixed CommonMark gap, but Inkmark remains faster in these engine comparisons. **The overall Inkmark performance target remains unmet:** README, reference-heavy, and GFM workloads also incur substantial grammar compatibility costs. GFM additionally has unequal output, as described above.

The repeated snapshot was prompted by timing variation: the unchanged final binary measured 4.211 ms in a separate diagnostic repeat, compared with roughly 5 ms in the engine runs. Its saved pre-validation comparator measured 4.730 ms in that repeat. Background media analysis and indexing were active during portions of the first run. Both snapshots and every pair are preserved; the diagnostic repeat does not establish a new engine ranking.

For historical context, the previous grammar-backed pull implementation measured 34.500 ms on mixed 1 MB ([dispatch measurements](measurements/2026-10-23-dispatch/)); the original implementation measured 551.1 ms in the first paired study ([original measurements](measurements/2026-10-23/)). Those are separate source snapshots and runs, not a new paired comparison. The intermediate [follow-up](measurements/2026-10-23-followup/) and [initial pull](measurements/2026-10-23-pull/) evidence remains unchanged.

Focused fixtures expose both strengths and remaining gaps. These are complete Markdown-to-HTML conversions, not isolated byte-scanner timings:

| Fixture | core-markdown | Inkmark |
| --- | ---: | ---: |
| long-lines-1m | 0.128 | 0.574 |
| code-1m | 0.073 | 0.310 |
| references | 5.187 | 0.842 |
| linked-attention | 0.850 | 0.919 |

## Pull architecture and allocation costs

The default HTML API consumes `EventParser` through `EventHtmlRenderer`. `Document` builds a contiguous arena from that iterator; `AstBuilder::consume` can stop after a caller-selected event budget. Arena traversal feeds the same HTML writer without allocating. Ordinary text, code, and attributes retain source byte ranges. Editing a value materializes that value alone. See the [architecture and contract](../../context/pull-parser.md).

Ordinary CommonMark uses compact block records and reusable inline candidates. Blocks are prepared first; inline resolution occurs only when their events are requested. Ordinary same-line reference definitions also use compact blocks. Raw HTML blocks, unsupported definition forms, extension configurations, and disabled constructs select the grammar compatibility producer. That producer can still use compact inline resolution for eligible CommonMark paragraphs. Multiline code/resources, inline HTML, NUL handling, and inline-code language metadata retain per-region grammar fallback. The legacy mdast API remains separate.

The following diagnostic times include preparation and disposal in every row. They are independent complete operations, not additive phases:

| Complete operation | README 9 KB | Mixed 100 KB | Mixed 1 MB | GFM 100 KB |
| --- | ---: | ---: | ---: | ---: |
| Prepare blocks and dispose | 0.052 | 0.091 | 0.929 | 3.007 |
| Prepare, pull all events, and dispose | 0.084 | 0.312 | 3.181 | 4.969 |
| Parse and render HTML | 0.088 | 0.494 | 4.968 | 5.049 |
| Parse, build arena, render HTML, and dispose | 0.103 | 0.921 | 10.759 | 5.875 |

On mixed 1 MB, the arena contains 125,784 nodes at 128 bytes per node on arm64. Fresh allocation counts are:

| Phase | Allocation calls | Reallocations | Cumulative requested MiB |
| --- | ---: | ---: | ---: |
| Block preparation | 3 | 14 | 6.22 |
| Pulling after preparation | 8 | 3 | 0.00 |
| Complete arena construction | 13 | 33 | 38.22 |
| Arena traversal | 0 | 0 | 0.00 |
| Complete HTML conversion | 13 | 36 | 10.22 |
| HTML with reserved output | 12 | 18 | 6.22 |
| Legacy mdast construction | 364,576 | 38,749 | 453.67 |

The previous grammar-backed pull path made **241,357 allocation calls plus 15,614 reallocations** for complete HTML conversion. Compact blocks and reused inline storage remove that per-line/per-event allocation volume. The parser still needs growing block, delimiter, and output buffers; this is not a strictly heap-free parser. Reallocations are reported separately from allocation calls. Requested bytes include every growth request and do not measure peak or retained memory. Input loading and options construction are excluded. Reserved-output rendering uses the checked public event API, while direct HTML uses its internal parser invariants.

## Isolated optimization studies

Each optimization has a separate commit. Three pairs per study compare successive sources with identical benchmark adapters; each pair runs baseline then candidate. Unlike the final engine comparison above, these study pairs do not alternate order. Reported ranges are the three process medians, in milliseconds. Percentages across studies must not be added.

| Change | Workload | Before | After |
| --- | --- | ---: | ---: |
| Inline hot event transfers and small HTML writes | Mixed 1 MB | 5.361–5.413 | 5.168–5.225 |
| Borrow semantic events through HTML rendering | Mixed 1 MB | 5.461–5.522 | 5.364–5.401 |
| Inline compact parser iteration across the event boundary | Mixed 1 MB | 5.345–5.374 | 4.949–5.052 |
| Inline event dispatch into the shared rendering loop | Mixed 1 MB | 4.790–4.953 | 4.573–4.730 |
| Keep ordinary leading inline links on the compact block path | Formatted links 96 KB | 1.792–1.816 | 0.729–0.749 |
| Avoid repeated range validation for the internal parser | Mixed 1 MB | 4.579–4.685 | 4.104–4.145 |

The last optimization constructs the parser from the renderer's own source. Public event consumers and AST edits retain range validation; the internal path uses the same renderer, balance checks, and safe string resolution. Tests compare direct HTML, checked events, and arena rendering.

Earlier optimizations index and batch grammar event edits, compact internal state and events, bypass disabled recognizers, reuse tokenizer scratch buffers, binary-search Unicode/entity tables, escape HTML and URI runs directly into output, and avoid cloned ancestor stacks. Their isolated measurements remain in the historical directories above. Experiments that slowed rendering were discarded.

The compact block scanners and algorithms adapt [pulldown-cmark 0.13.4](https://github.com/pulldown-cmark/pulldown-cmark/tree/38e4d08f14ec4bd9783270e9623db7681ebed968), retaining MIT notices. Inline backticks are indexed by run length, emphasis uses bounded opener searches, and link-openers use a monotone invalidation boundary. A bounded AArch64 NEON two-nibble lookup scans inline and HTML-escape candidates, with a portable scalar fallback; tests compare every byte across vector boundaries and alignments. `memchr` supplies additional portable byte searches with default features disabled. The library retains `no_std` support and does not depend on a second Markdown backend. [Inkmark's release lockfile](https://github.com/yaroslav/inkmark/blob/v0.2.1/Cargo.lock) resolves pulldown-cmark 0.13.4.

## Output consistency

Fresh byte-for-byte checks of final direct HTML output produced the following counts. GFM measures agreement with core-markdown, rather than treating the old published reference output as a conformance oracle under globally enabled extensions.

| Engine | Exact CommonMark reference matches, of 652 | Exact GFM agreement with core-markdown, of 677 |
| --- | ---: | ---: |
| core-markdown | 652 | 677 |
| markdown-rs | 652 | 677 |
| Comrak | 652 | 667 |
| Markly | 640 | 661 |
| Commonmarker | 652 | 667 |
| Inkmark | 630 | 634 |

Byte differences do not necessarily imply different HTML meaning. Inkmark's 22 CommonMark differences are formatting and equivalent quote escaping; the original comparison's normalizer, extended to equate literal text quotes with `&quot;`, found agreement on all 652. Markly's differences include numeric entities, emphasis boundaries, and an email autolink. GFM differences additionally include tag filtering, table serialization, and literal autolinks. Footnotes are outside the timed GFM profile; their IDs and markup vary across implementations.

The final source preserves all **2,117 saved original outputs** across direct HTML, checked event rendering, and arena rendering: 652 CommonMark examples, 677 published GFM examples, 672 separate cmark-gfm examples, and 29 targeted inputs in four profiles. [Preservation records](measurements/2026-10-23-definitions/preservation.json) include input and output hashes. The architecture and release notes document intentional corrections outside that saved corpus: heading IDs, malformed container closures, tabs and unclosed fences, mixed line-ending separators, frontmatter rollback, escaped emphasis, reference-label spaces, adjacent setext headings, and retained HTML list boundaries. The legacy mdast renderer retains its earlier representation and is not claimed to match the shared writer in every case.

Independent review exercised randomized and structured inputs, construct configurations, stream/arena parity, and raw nesting. Regression tests cover bounded AST construction, early drop, source detachment, malformed public streams, writer failures, extension metadata, and scanner boundaries. Final local coverage passed **25,925/25,925 measured regions** on arm64 with all features (68 invariant-only `unreachable!` regions excluded by the shared gate); the portable x86_64 path also cross-compiles without default features. These checks establish coverage and observed agreement, not a proof for all inputs.

## Remaining work

The next performance work is extending compact block preparation to raw HTML and the remaining definition forms, avoiding copies of constructed reference values and Unicode normalization intermediates, and supporting GFM directly in the compact representation. Same-line definitions and reference resolution now use compact storage, as described in the follow-up measurements above. Raw HTML comments in the saved README still select grammar block preparation. README and GFM account for the clearest remaining gaps against Inkmark. Platform-specific SIMD beyond AArch64 also remains a possible measured optimization.

No public feature was removed. The old direct HTML token compiler and unused internal attention wrappers were removed; MDX remains opt-in, with disabled recognizers bypassed and diagnostics moved out of hot parser states. The pull API accepts a complete UTF-8 buffer. It does not stream input chunks or incrementally reparse edited source.

Sources: [CommonMark 0.31.2 examples](https://spec.commonmark.org/0.31.2/spec.json), [published GFM specification](https://github.github.com/gfm/), [markdown-rs](https://github.com/wooorm/markdown-rs), [Comrak](https://github.com/kivikakk/comrak), [Markly](https://github.com/socketry/markly), [Commonmarker](https://github.com/gjtorikian/commonmarker), and [Inkmark](https://github.com/yaroslav/inkmark).
