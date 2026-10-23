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

The JSON input is an array of objects containing `markdown` and, for timings, `name`. `core-arena` measures parsing into the source-backed document and rendering its event traversal. `core-ast` measures the separate legacy mdast/HTML-renderer path. Neither is the direct-HTML path in the six-engine table. `prepare.py` also generates `bench-scanners.json` (long prose and fenced code), `bench-references.json`, and `bench-linked-attention.json` for focused diagnostics.

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

Measured on 2026-10-23 using an Apple M4 Pro, macOS 27.0.1 arm64, Rust 1.99.1, and Ruby 4.0.7 with YJIT disabled. The Rust engines use release builds without syntax highlighting; Commonmarker and Inkmark use the published arm64-darwin binaries, and Markly is built locally. Versions are markdown-rs 1.0.0, Comrak 0.57.0, Markly 0.19.0, Commonmarker 2.10.1, and Inkmark 0.2.1. Commonmarker embeds the Comrak 0.56 series, so its underlying parser version differs from the standalone adapter.

Times are **milliseconds per conversion; lower is faster**.

| Engine | README 9 KB | Mixed 100 KB | Mixed 1 MB | GFM 100 KB |
| --- | ---: | ---: | ---: | ---: |
| core-markdown (pull stream) | 0.120 | 3.749 | 40.757 | 5.981 |
| markdown-rs 1.0.0 | 0.378 | 14.251 | 564.816 | 23.843 |
| Comrak 0.57.0 | 0.031 | 0.932 | 9.214 | 1.384 |
| Markly 0.19.0 | 0.043 | 1.056 | 12.460 | 1.625 |
| Commonmarker 2.10.1 | 0.044 | 1.104 | 11.076 | 1.634 |
| Inkmark 0.2.1 | 0.019 | 0.405 | 4.319 | 0.710 |

The pull-stream measurement uses production source `7ed5e9d`. Raw samples, supplemental studies, source attribution, and input hashes are preserved in [measurements/2026-10-23-pull](measurements/2026-10-23-pull/). Earlier samples remain unchanged in [measurements/2026-10-23-followup](measurements/2026-10-23-followup/) and [measurements/2026-10-23](measurements/2026-10-23/).

The earlier paired comparison measured the original implementation at `e5effec59c852861b7b5e96eaf3eefe9e5a6e09b` against the optimized direct compiler at `4f614d0`: the 1 MB fixture improved from 551.1 ms to 41.9 ms (**13.2× faster**). Growing that input by approximately 10× increased optimized time by 11.0×, versus 40.4× before. This repaired the conspicuous scaling problem on this workload; it is not a worst-case complexity proof. The new pull architecture has comparable end-to-end mixed-document throughput to that optimized compiler; these separate runs do not establish a further throughput gain.

Three further optimizations move fatal diagnostics out of copyable tokenizer states, bypass impossible block recognizers on ordinary unindented lines, and identify emphasis contexts without cloning ancestor stacks. A fresh two-engine run at `6bc8f57` measures:

| Engine | README 9 KB | Mixed 100 KB | Mixed 1 MB | GFM 100 KB |
| --- | ---: | ---: | ---: | ---: |
| core-markdown | 0.104 | 3.134 | 34.500 | 5.143 |
| Inkmark 0.2.1 | 0.018 | 0.398 | 4.249 | 0.696 |

The follow-up samples and paired studies are preserved separately in [measurements/2026-10-23-dispatch](measurements/2026-10-23-dispatch/). The six-engine table above retains its original source and samples; the two runs should not be combined into a new six-engine ranking.

**The Inkmark performance target remains unmet.** The latest two-engine run has a remaining **8.1× gap** on the 1 MB fixture. Block preparation still uses the existing grammar tokenizer and remains expensive. The shared event contract removes the need to collect the semantic stream or construct an AST for HTML, but is not yet a zero-allocation parser or a replacement for block token production.

## Pull architecture and focused measurements

The default HTML API now consumes `EventParser` through `EventHtmlRenderer`. `Document` incrementally builds a contiguous arena from the same iterator, and arena traversal feeds that same HTML writer. Source text retains byte offsets; editing a value materializes that value alone. See the [architecture and contract](../../context/pull-parser.md). The legacy mdast API remains available.

`memchr` supplies portable SIMD-assisted searches for ordinary block, code, and math runs. There is no custom unsafe scanner and the library retains `no_std` support. Three alternating scalar/SIMD pairs measured long prose at 1.72–1.89 ms versus 1.24–1.30 ms, and fenced code at 1.16–1.38 ms versus 0.62–0.74 ms. The mixed 1 MB fixture was essentially unchanged at 38.4–39.1 ms in that isolated study. Those earlier-source scanner results are supplemental diagnostics, not the final six-engine timings.

Compacting tokenizer rollback checkpoints from a complete position to the necessary source index reduced the isolated mixed fixture from 40.3–40.5 ms to 39.3–39.4 ms. Folding ASCII reference labels in place removes two temporary string allocations per normalization: the reference-heavy fixture improved from 5.99–6.10 ms to 5.73–5.75 ms, approximately 5%; its mixed-document results were noisy. Two additional event-edit storage/reuse experiments were discarded after slowing mixed-document rendering by approximately 4–12%.

Three alternating pairs for each follow-up optimization show:

- Keeping diagnostics on the tokenizer, outside its returned state, reduces mixed 1 MB rendering from 39.57–39.83 ms to 37.22–37.45 ms, approximately 6%.
- Sending ordinary unindented lines directly to table or paragraph recognition reduces mixed rendering from 37.71–39.76 ms to 36.21–36.30 ms. The first baseline run was slower; the later pairs show approximately 4% improvement. Whitespace and possible block markers retain ordered recognition, including enabled MDX.
- Replacing each emphasis delimiter's cloned ancestor stack with its innermost opening-event index reduces mixed rendering from 35.82–35.99 ms to 34.73–35.04 ms. A separate 96 KB formatted-link fixture improves from 5.60–5.62 ms to 5.24–5.30 ms and removes 12,000 allocation calls from complete HTML conversion (82,041 → 70,041). A unique opening event identifies its complete ancestor chain, preserving delimiter boundaries between links and images.

These are isolated studies on successive sources, not additive percentages. The mixed fixture's allocation-call count is unchanged by these three optimizations. A qualitative allocation trace still points primarily to per-line event splitting and small temporary event vectors for paragraph boundaries, document exits, and list wrappers.

A separate diagnostic run of parser → arena → HTML took 0.128 ms for the README, 4.081 ms for mixed 100 KB, 45.146 ms for mixed 1 MB, and 6.497 ms for GFM 100 KB. These include arena construction and output generation; traversal alone is not being timed.

On the mixed 1 MB fixture, the arena has 125,785 nodes at 128 bytes per node on arm64. Traversing it allocates nothing. At the pull measurement's `33f9c0f` allocation baseline, the full parser still allocates heavily:

| Phase | Allocation calls | Reallocations | Cumulative requested MiB |
| --- | ---: | ---: | ---: |
| Block preparation | 118,127 | 189 | 265.2 |
| Pulling after preparation | 118,094 | 15,406 | 90.9 |
| Complete arena construction | 236,223 | 15,611 | 388.0 |
| Arena traversal | 0 | 0 | 0 |
| Complete HTML conversion | 241,357 | 15,614 | 360.1 |
| HTML with reserved output | 241,356 | 15,596 | 356.1 |
| Legacy mdast construction | 408,216 | 38,739 | 466.7 |

Construction and conversion rows are independent complete operations, not additive phases. Requested bytes include repeated requests for growing buffers and must not be read as resident memory. Input storage and output preallocation are outside their respective measurement windows. The large preparation and pulling counts quantify why a source-backed semantic API alone does not achieve the allocation target; ordinary source text and arena traversal already avoid per-value copies.

## Output consistency

These counts are fresh byte-for-byte comparisons of the direct HTML paths. GFM counts measure agreement with core-markdown, rather than treating the old published reference's output as a conformance oracle under globally enabled extensions.

| Engine | Exact CommonMark reference matches, of 652 | Exact GFM agreement with core-markdown, of 677 |
| --- | ---: | ---: |
| core-markdown | 652 | 677 |
| markdown-rs | 652 | 677 |
| Comrak | 652 | 667 |
| Markly | 640 | 661 |
| Commonmarker | 652 | 667 |
| Inkmark | 630 | 634 |

Byte differences do not necessarily imply different HTML meaning. Inkmark's 22 CommonMark differences are formatting and equivalent quote escaping; the earlier comparison's CommonMark reference normalizer, extended to equate literal text quotes with `&quot;`, found agreement on all 652. Markly's differences include numeric entities, emphasis boundaries, and an email autolink. GFM differences additionally include tag filtering, table serialization, and unusual literal autolinks. Footnote IDs and markup vary across implementations and are outside the timed GFM profile. Defaults also differ substantially; in particular, Commonmarker's default syntax highlighting and presentation features are disabled here.

The final pull implementation preserves all 2,117 saved core-markdown outputs from the original comparison, and its arena rendering matches them as well (see [preservation.json](measurements/2026-10-23-pull/preservation.json)): 652 CommonMark examples, 677 published GFM examples, 672 examples from the separate cmark-gfm corpus, and 29 targeted inputs in four profiles. An independent review of the first stage also compared 10,000 randomized Markdown inputs under three profiles and 4,096 construct configurations, including full mdast positions. Review of the final fast paths additionally compared 50,000 randomized inputs and 65,536 construct configurations against the original parser, including exact HTML and full AST positions. The pull implementation additionally exercises parser → HTML and parser → arena → HTML parity, bounded construction, source detachment, malformed public streams, writer failures, and extension metadata. Independent review included one million randomized inputs, 65,536 option configurations, and sampled arena roundtrips throughout that review before the final focused fixes. The final-source corpus agreement is recorded above; intentionally corrected heading IDs, malformed container closures, speculative frontmatter positions, and reference-label whitespace are documented in the architecture and release notes. The legacy mdast HTML renderer retains pre-existing differences and is not claimed to match the shared writer.

## Optimizations and remaining work

The first fifteen optimizations each have their own commit:

1. Index pending event edits instead of repeatedly scanning all earlier edits. This removes the dominant superlinear behavior on the repeated-section fixture.
2. Apply edits with one pass over the source event buffer, replacing repeated split allocations and copies.
3. Binary-search the sorted Unicode punctuation table, with an ASCII shortcut.
4. Scan ordinary byte runs without dispatching a tokenizer state for every byte; retain existing handling at tabs, line endings, and linked-chunk boundaries.
5. Binary-search named character references, with exhaustive table-order/lookup checks.
6. Box rare MDX diagnostics to reduce the size of every parser state.
7. Use one-based nonzero link indices, reducing private events from 80 to 64 bytes on this machine.
8. Escape HTML directly into the active output buffer.
9. Reuse tokenizer scratch buffers between subcontent chunks.
10. Represent enabled inline delimiters with an ASCII bitset, avoiding disabled extension markers in ordinary prose.
11. Drop six unused internal attention-wrapper events per resolved emphasis/strong/strikethrough match. This reduces intermediate event volume; isolated timing did not establish an additional throughput gain on the mixed fixture.
12. Check tokenizer state variants directly instead of invoking derived structural equality.
13. Reuse flow line chunks to form ordinary paragraphs when a definition cannot start, avoiding a redundant content-tokenizer pass. Inputs starting with a bracket or indentation retain definition parsing.
14. Inspect CRLF event boundaries directly instead of repeating byte normalization and tab-expansion checks.
15. Scan ordinary code and math byte runs without redispatching a tokenizer state for every byte; keep marker, tab, line-ending, and linked-chunk boundaries.

The delimiter bitset and byte-run scanning follow the general approach used by [pulldown-cmark's scanners](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/scanners.rs). No competitor source was copied. [Inkmark's native dependency manifest](https://github.com/yaroslav/inkmark/blob/v0.2.1/ext/inkmark/Cargo.toml) enables pulldown-cmark's HTML and SIMD features; its [release lockfile](https://github.com/yaroslav/inkmark/blob/v0.2.1/Cargo.lock) resolves pulldown-cmark 0.13.4.

Subsequent commits introduce the semantic event contract and arena, defer inline resolution and reuse its buffers, replace the default HTML compiler with the shared writer, escape URI runs directly into the writer, add SIMD-assisted scanning, compact arena links and tokenizer checkpoints, and fold ASCII labels in place. Correctness fixes and measurements have separate commits.

The follow-up commits keep fatal diagnostics outside copyable states, skip impossible block recognizers on ordinary lines, and track attention context identity without cloning ancestor stacks. Independent comparison covered 40,960 flow document/option combinations and 32,768 attention combinations, including exact HTML, diagnostics, and full mdast positions. The final source preserves the 2,117 saved outputs and arena/direct parity, with 23,019 of 23,019 measured regions covered.

No public feature was removed. The `memchr` runtime dependency has default features disabled. Several earlier line-searching and storage prototypes did not justify their cost on the mixed workload and were discarded. Removed grammar wrapper events were private and unused by either compiler. MDX remains opt-in; ordinary-line dispatch now bypasses its unmatchable recognizers, and its fatal diagnostics no longer occupy every returned state. Feature separation or removal remains an option if a disabled cost cannot be isolated. The remaining substantial work is replacing expensive block grammar events with compact source-backed block records, reducing speculative tokenizer state and edit-buffer allocations, and avoiding normalization/destination clones during repeated link resolution. Those remain unimplemented optimization opportunities. The pull API accepts a complete UTF-8 source; it does not yet parse input chunks or incrementally reparse source edits.

Sources: [CommonMark 0.31.2 examples](https://spec.commonmark.org/0.31.2/spec.json), [published GFM specification](https://github.github.com/gfm/), [markdown-rs](https://github.com/wooorm/markdown-rs), [Comrak](https://github.com/kivikakk/comrak), [Markly](https://github.com/socketry/markly), [Commonmarker](https://github.com/gjtorikian/commonmarker), and [Inkmark](https://github.com/yaroslav/inkmark).
