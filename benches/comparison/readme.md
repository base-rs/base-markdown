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

`run.py` runs all six engines sequentially and saves raw batch timings in `results/`. Supply parser names to time a subset, for example `python3 run.py core-markdown inkmark`. `check.py` checks exact core-markdown HTML against all 652 CommonMark 0.31.2 reference examples and the pinned markdown-rs version on all 677 published GFM examples, plus 29 targeted inputs under four profiles. `--all` additionally records the six engines' agreement counts and complete outputs in `results/`. Downloaded specification sources are checked against recorded SHA-256 hashes; an unexpected source change requires inspection. The CommonMark and GFM specifications are attributed to their respective authors and licensed CC BY-SA 4.0; downloaded copies remain under ignored `data/`.

Individual Rust adapters support `bench`, `render`, and a 30-second `profile` loop:

```sh
target/release/markdown-comparison render core-markdown commonmark data/commonmark.json
target/release/markdown-comparison profile core-markdown commonmark data/bench-commonmark.json
bundle exec ruby adapter.rb inkmark render commonmark data/commonmark.json
```

The JSON input is an array of objects containing `markdown` and, for timings, `name`. `core-ast` is an additional diagnostic adapter for the separate mdast/HTML-renderer path; it is not the direct-HTML path measured below.

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
| core-markdown (baseline) | 0.352 | 13.642 | 551.117 | 22.852 |
| core-markdown (optimized, paired) | 0.132 | 3.801 | 41.874 | 5.951 |
| core-markdown (optimized) | 0.129 | 3.750 | 41.689 | 5.903 |
| markdown-rs 1.0.0 | 0.371 | 13.867 | 553.357 | 23.106 |
| Comrak 0.57.0 | 0.030 | 0.911 | 9.118 | 1.342 |
| Markly 0.19.0 | 0.041 | 1.086 | 12.066 | 1.579 |
| Commonmarker 2.10.1 | 0.044 | 1.089 | 10.817 | 1.601 |
| Inkmark 0.2.1 | 0.019 | 0.397 | 4.230 | 0.703 |

The baseline is core-markdown at `e5effec59c852861b7b5e96eaf3eefe9e5a6e09b`. Its package version was changed only in a temporary diagnostic worktree to allow baseline and optimized packages in one executable; its implementation was unchanged. The optimized measurements use production source `4f614d0`. Paired results use separate release executables, run sequentially on the same host; the six-engine table is another sequential run. Raw samples and input hashes are preserved in [measurements/2026-10-23-followup](measurements/2026-10-23-followup/). The earlier eleven-optimization stage remains in [measurements/2026-10-23](measurements/2026-10-23/) without changing its samples or source attribution. To reproduce a baseline run without changing its package version, create a detached worktree at that commit, extract this PR's `benches/comparison/` directory into it, then run `python3 run.py core-markdown` there.

The paired 1 MB run improves from 551.1 ms to 41.9 ms (**13.2× faster**). The README, mixed 100 KB, and GFM fixtures improve by 2.7×, 3.6×, and 3.8× respectively. Increasing the mixed input by approximately 10× increases optimized paired time by 11.0×, versus 40.4× before. This repairs the conspicuous scaling problem on this workload; it is not a worst-case complexity proof.

**The Inkmark performance target remains unmet.** In the fresh six-engine run, optimized core-markdown takes 41.69 ms on the 1 MB fixture versus Inkmark’s 4.23 ms, a remaining 9.9× gap. The paired and standalone optimized results also illustrate normal inter-run variation. Further work needs to address the remaining event/tokenizer architecture rather than extrapolate another order of magnitude from these local improvements.

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

All 2,117 saved core-markdown outputs from the original comparison remained byte-identical after the optimizations: 652 CommonMark examples, 677 published GFM examples, 672 examples from the separate cmark-gfm corpus, and 29 targeted inputs in four profiles. An independent review of the first stage also compared 10,000 randomized Markdown inputs under three profiles and 4,096 construct configurations, including full mdast positions. Review of the final fast paths additionally compared 50,000 randomized inputs and 65,536 construct configurations against the original parser, including exact HTML and full AST positions. These checks supplement the repository's regression suite and complete measured source-region coverage. The separate AST HTML renderer has pre-existing output differences and is not claimed to match the direct compiler.

## Optimizations and remaining work

Each optimization has its own commit:

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

No public feature was removed, and no runtime dependency was added. Accelerated line searching, alternate buffer ownership, and deletion-only compaction prototypes did not show worthwhile throughput gains on the mixed workload and were discarded. The low-value grammar wrapper events removed here were private and unused by either compiler. Dropping MDX or math support is not justified by these measurements: disabled markers now stay out of the CommonMark hot path, and rare diagnostics no longer inflate state storage. More substantial candidates are emitting fewer grammar events throughout the parser, borrowing source spans instead of materializing intermediates, and reducing whole-event-buffer rewrites. These are architectural proposals, not measured speedups. An alternate HTML backend would need explicit compatibility decisions for GFM, custom construct flags, callbacks, diagnostics, source positions, and `no_std` before it could replace the current path.

Sources: [CommonMark 0.31.2 examples](https://spec.commonmark.org/0.31.2/spec.json), [published GFM specification](https://github.github.com/gfm/), [markdown-rs](https://github.com/wooorm/markdown-rs), [Comrak](https://github.com/kivikakk/comrak), [Markly](https://github.com/socketry/markly), [Commonmarker](https://github.com/gjtorikian/commonmarker), and [Inkmark](https://github.com/yaroslav/inkmark).
