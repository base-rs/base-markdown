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

The JSON input is an array of objects containing `markdown` and, for timings, `name`. `core-prepare` times parser construction and disposal; `core-events` additionally drains resolved events into `black_box`, without rendering. `core-arena` measures parsing into the source-backed document and rendering its event traversal. `core-ast` measures the separate legacy mdast/HTML-renderer path. These diagnostic modes are separate from the direct-HTML path in the six-engine table. `prepare.py` also generates `bench-scanners.json` (long prose and fenced code), `bench-references.json`, and `bench-linked-attention.json` for focused diagnostics.

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

## Measurements

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

Ordinary CommonMark uses compact block records and reusable inline candidates. Blocks are prepared first; inline resolution occurs only when their events are requested. Reference-definition candidates, raw HTML blocks, extension configurations, and disabled constructs select the grammar compatibility producer. That producer can still use compact inline resolution for eligible CommonMark paragraphs. Multiline code/resources, inline HTML, NUL handling, and inline-code language metadata retain per-region grammar fallback. The legacy mdast API remains separate.

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
| Inline event dispatch into the shared rendering loop | Mixed 1 MB | 4.790–4.953 | 4.573–4.729 |
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

The final source preserves all **2,117 saved original outputs** across direct HTML, checked event rendering, and arena rendering: 652 CommonMark examples, 677 published GFM examples, 672 separate cmark-gfm examples, and 29 targeted inputs in four profiles. [Preservation records](measurements/2026-10-23-compact/preservation.json) include input and output hashes. The architecture and release notes document intentional corrections outside that saved corpus: heading IDs, malformed container closures, tabs and unclosed fences, mixed line-ending separators, frontmatter rollback, escaped emphasis, reference-label spaces, adjacent setext headings, and retained HTML list boundaries. The legacy mdast renderer retains its earlier representation and is not claimed to match the shared writer in every case.

Independent review exercised randomized and structured inputs, construct configurations, stream/arena parity, and raw nesting. Regression tests cover bounded AST construction, early drop, source detachment, malformed public streams, writer failures, extension metadata, and scanner boundaries. Final local coverage passed **25,501/25,501 measured regions** on arm64 with all features (68 invariant-only `unreachable!` regions excluded by the shared gate); the portable x86_64 path also cross-compiles without default features. These checks establish coverage and observed agreement, not a proof for all inputs.

## Remaining work

The next performance work is extending compact block preparation to reference definitions and raw HTML, resolving reference links without repeated normalization/destination copies, and supporting GFM directly in the compact representation. These paths account for the clearest remaining gaps against Inkmark. Platform-specific SIMD beyond AArch64 also remains a possible measured optimization.

No public feature was removed. The old direct HTML token compiler and unused internal attention wrappers were removed; MDX remains opt-in, with disabled recognizers bypassed and diagnostics moved out of hot parser states. The pull API accepts a complete UTF-8 buffer. It does not stream input chunks or incrementally reparse edited source.

Sources: [CommonMark 0.31.2 examples](https://spec.commonmark.org/0.31.2/spec.json), [published GFM specification](https://github.github.com/gfm/), [markdown-rs](https://github.com/wooorm/markdown-rs), [Comrak](https://github.com/kivikakk/comrak), [Markly](https://github.com/socketry/markly), [Commonmarker](https://github.com/gjtorikian/commonmarker), and [Inkmark](https://github.com/yaroslav/inkmark).
