# Markdown parser comparison

This harness compares the public Markdown-to-HTML APIs of core-markdown, markdown-rs, Comrak, Markly, Commonmarker, and Inkmark. It lives outside the main workspace so competitor dependencies do not affect the published crate or ordinary tests. The adjacent [Rust](Cargo.lock) and [Ruby](Gemfile.lock) lockfiles pin the comparison dependencies.

## Recorded comparison

The latest [footnote, definition-index and escape-context consolidation](measurements/2026-10-25-footnotes-indexes-and-escape-context/readme.md) borrows serializer context and writes footnote backlinks through the shared HTML writer. Prepared-document Markdown takes 24.6–32.3% less time on the original four, and footnote controls take 14.7–34.1% less time. Original direct HTML ranges from 0.4% less to 1.3% more time. Fenced-body reservation removes a measured reallocation; all initial regressions, final results and process spreads are retained in the report. This pass does not retime Inkmark.

The earlier [scratch rewrite and scanner consolidation](measurements/2026-10-25-serializer-rewrites-and-lookahead/readme.md) shares inline normalization, marker runs and conservative text lookahead. Prepared-document Markdown takes 5.0–8.3% less time on the original four; code-fence/math/table controls take 46.9–95.3% less time. Complete HTML ranges from 0.4% less to 1.8% more time; the mixed-1m regression is retained in the report. Output and diagnostics are preserved, with fewer scratch allocations on applicable controls. This pass does not retime Inkmark.

The earlier [serializer and AST edit consolidation](measurements/2026-10-25-serializer-and-edits/readme.md) removes 125 net production lines. Section-copy controls are 6.5–71.2× faster and paired subtree moves are 2.0–2.4× faster than #55; HTML conversion and prepared-document Markdown serialization remain within approximately 1.2%. Allocation counts fall for section copying and remain unchanged for the other measured operations. It does not retime Inkmark.

The subsequent [parser consolidation study](measurements/2026-10-25-parser-consolidation/readme.md) compares production `561ae1d` with #54 after five simplification rounds: 53 fewer net production lines, unchanged allocations, and identical complete events on 58,596 probes. The original four range from 0.61% less time to 0.67% more in its primary run; mixed-1m is 0.62% slower and 0.65% slower in a separate repeat. That pass does not retime Inkmark; the table below remains the recorded #54 comparison.

The [inline destination study](measurements/2026-10-25-inline-destinations/readme.md) compares production **`08836d8a3474f220e60c09a90be17d8ae259d07a`** with squash-merged #53 and Inkmark 0.2.1. [Metadata](measurements/2026-10-25-inline-destinations/metadata.json) pins binaries, sources, compiler, locks and inputs.

The original four workloads, in milliseconds per complete conversion; lower is faster:

| Workload | Input bytes | core-markdown ms | Inkmark ms |
| --- | ---: | ---: | ---: |
| README | 9,019 | 0.013416 | 0.019405 |
| Mixed 100 KB | 100,252 | 0.393345 | 0.415127 |
| Mixed 1 MB | 1,000,020 | 3.931404 | 4.457829 |
| GFM 100 KB | 100,130 | 0.621221 | 0.720311 |

The shared byte scanner skips ordinary inline link destination runs without changing allocations or output. Clap's README takes **14.9% less time** and Awesome Rust's README **18.2% less time** than #53 in the primary cohort; a separate repeat finds 14.2% and 16.2% less time. The original four take 0.4–1.8% less time. The CommonMark specification takes **2.2% more time** in the primary cohort and 0.2% less in the repeat; small effects remain uncertain on this shared host.

Core-markdown has lower medians than Inkmark on all twelve established workloads and all six pinned real documents in this run. It has twenty-eight lower medians across all twenty-nine inputs; mixed dense-Unicode keys remain **4.1% behind Inkmark**. Real documents agree after entity decoding. **Generated GFM output differs:** Inkmark leaves `www.` unlinked and table formatting differs. These shared-host measurements do not establish a universal ranking; candidate Awesome Rust process medians span 56.2%. See the [complete tables, repeats and raw samples](measurements/2026-10-25-inline-destinations/readme.md) before generalizing.

The [preceding Unicode folding study](measurements/2026-10-25-normalize-identifiers/readme.md) retains its casing allocation gains, late-uppercase tradeoff, and initial real-document comparison.

## Reproduction

Install a Rust toolchain, Python 3, Ruby, Bundler, and a compiler for the native gems. From the repository root:

```sh
cd benches/comparison
bundle config set --local path vendor/bundle
bundle install
python3 run.py core-markdown inkmark
python3 check.py --all
```

Omit parser names from `run.py` to run all six engines. It regenerates deterministic inputs with [prepare.py](prepare.py), builds the release adapter with `--locked`, runs the engines sequentially, and writes raw batch timings to `results/`. The original mixed workloads repeat prose, emphasis, links, inline code, Unicode, entities, block quotes, nested lists, and fenced Rust code; the GFM workload adds tables, tasks, and literal autolinks. Additional generated inputs isolate long prose, code, references, definitions, and linked emphasis.

To reproduce a saved study, use its recorded source revisions, toolchain, lockfiles, and measurement driver. Keep immutable executables outside Cargo target directories, alternate engine order across rounds, and keep builds, tests, profiling, and allocation diagnostics outside the timing window. The [recorded study instructions](measurements/2026-10-25-event-delivery/readme.md#validation-and-reproduction) document its three-round run; the quick command above performs one process run per engine/profile.

The Rust adapter also accepts `render` and a 30-second `profile` loop:

```sh
target/release/markdown-comparison render core-markdown commonmark data/commonmark.json
target/release/markdown-comparison profile core-markdown commonmark data/bench-commonmark.json
bundle exec ruby adapter.rb inkmark render commonmark data/commonmark.json
```

Inputs are JSON arrays containing `markdown` and, for timing, `name`. Diagnostic engines measure complete operations, including disposal: `core-prepare` constructs and drops the parser; `core-events` additionally consumes every semantic event without rendering; `core-arena` builds a `Document` and renders its event traversal; `core-markdown-output` builds a document and serializes Markdown. `core-ast` aliases `core-arena`. These operations are independent measurements, not additive phases.

## Method and output checks

Each process warms a document for at least 200 ms, then runs seven batches calibrated to approximately 150 ms each. Its result is the median batch time per complete conversion. The recorded table takes the median of three alternating process runs. Startup, library loading, input I/O, JSON transport, and initial options construction are excluded; parsing, output generation, result disposal, and normal Ruby garbage collection are included. Neither rendered output nor parsed documents are cached.

The CommonMark profile disables extensions, permits raw HTML and all URL protocols, preserves soft breaks, and disables syntax highlighting. GFM adds tables, strikethrough, tasks, and literal autolinks, with footnotes disabled. Comrak and Commonmarker do not expose the same independent GFM tag filter as the other adapters. These permissive configurations match the fixture comparison.

[check.py](check.py) checks direct/document HTML parity, all 652 CommonMark 0.31.2 reference examples, agreement with pinned markdown-rs on all 677 published GFM examples, and 29 targeted inputs under four profiles. `--all` also saves each engine's complete outputs and agreement counts. Downloaded specification sources are verified against recorded SHA-256 hashes; unexpected changes require inspection. The specifications are attributed to their authors and licensed CC BY-SA 4.0; downloaded copies remain under ignored `data/`.

Byte differences can represent equivalent HTML escaping or formatting, or substantive differences in links, emphasis, tables, tag filtering, and footnotes. Read exact and normalized comparisons separately. The consolidation study preserves all 8,468 saved HTML/Markdown outputs and 58,596 diagnostic results against #54, including complete serialized event sequences. Its [output agreement](measurements/2026-10-25-parser-consolidation/output-agreement.json), [event preservation](measurements/2026-10-25-parser-consolidation/event-preservation.json) and [corpus preservation](measurements/2026-10-25-parser-consolidation/preservation.json) identify the checked inputs and hashes. The [single-parser study](measurements/2026-10-25-single-parser/readme.md#output-and-compatibility) separately records intentional corrections outside the saved corpus. Observed agreement is regression evidence, not a proof for every input.

## Architecture and allocations

One source-backed parser handles CommonMark and independently enabled GFM, math, and frontmatter constructs. Block preparation records structure and definitions; a reusable inline scanner resolves each region when its events are pulled. Direct HTML consumes those events without constructing an AST. `Document` is the sole AST and builds incrementally from the same contract; its event traversal feeds the shared HTML writer. Ordinary values retain source ranges, while discontinuous or normalized values materialize only when needed. See the [pull architecture](../../context/pull-parser.md) and [document API](../../context/document.md).

The implementation retains `no_std`, shared byte scanners with AArch64 NEON and portable scalar paths, and reusable parser/output buffers. It has no grammar compatibility backend or separate mdast renderer; MDX support has been removed. Input is a complete UTF-8 buffer: pulling events does not imply chunked input or incremental source reparsing.

The recorded source removes one allocation per contiguous canonical reference key: 4,096 fewer on the largest definition workload and four fewer on README. Event/node counts remain unchanged on all twelve paired allocation workloads. Its mixed 1 MB direct conversion makes 12 allocations and 27 reallocations; preparation makes 3 allocations and 14 reallocations, and pulling after preparation makes 7 allocations and 3 reallocations. Traversing a prepared document allocates nothing; document nodes measure 120 bytes on the recorded arm64 compiler. GFM and multiline normalization still allocate: the parser is not heap-free. [Complete allocation records](measurements/2026-10-25-event-delivery/allocations.json) include all measured operations.

Allocation diagnostics use a separate executable so timing retains the system allocator:

```sh
cargo build --release --locked
target/release/allocations commonmark data/bench-commonmark.json
target/release/allocations gfm data/bench-gfm.json
```

Counts exclude input loading and options construction. Allocation calls and reallocations are separate; cumulative requested bytes include growth requests and do not measure peak or retained memory. Reserved-output HTML, complete conversion, document construction, traversal, and serialization are separate operations. Match their profiles and source revisions when comparing reports.

## Study history

Historical directories retain their original measurements, source identities, validation, rejected experiments, and architecture descriptions. Claims about removed APIs or parser designs apply to those recorded revisions. Compare engines within a study; do not combine percentage gains from different hosts, toolchains, or runs.

- [Initial six-engine comparison](measurements/2026-10-23/) and [early follow-up](measurements/2026-10-23-followup/).
- [Pull API](measurements/2026-10-23-pull/), [compact block preparation](measurements/2026-10-23-compact/), and [dispatch](measurements/2026-10-23-dispatch/).
- [Reference resolution](measurements/2026-10-23-references/), [definition preparation](measurements/2026-10-23-definitions/), and [list/reference simplification](measurements/2026-10-24-streamline-lists-and-references/readme.md).
- [Line scans](measurements/2026-10-24-line-scans/readme.md), [inline HTML and escaping](measurements/2026-10-24-inline-html-and-escaping/readme.md), and [HTML escape output](measurements/2026-10-24-escape-output/readme.md).
- [Event delivery](measurements/2026-10-24-commonmark-events/readme.md), [HTML/GFM](measurements/2026-10-24-html-gfm/), [HTML writes](measurements/2026-10-24-html-writes/readme.md), and [text coalescing](measurements/2026-10-24-text-coalescing/readme.md).
- [Hot paths](measurements/2026-10-24-hot-paths/readme.md), [implementation streamlining](measurements/2026-10-24-streamline/readme.md), and [event/storage simplification](measurements/2026-10-24-simplify-events/readme.md).
- [MDX removal](measurements/2026-10-24-remove-mdx/readme.md), [dispatch recovery](measurements/2026-10-24-recover-throughput/readme.md), [single AST](measurements/2026-10-24-document-ast/readme.md), and [serializer/storage simplification](measurements/2026-10-24-streamline-serialization/readme.md).
- [Single parser](measurements/2026-10-25-single-parser/readme.md) and [unified-parser throughput recovery](measurements/2026-10-25-recover-single-parser/readme.md).

- [Reference labels and event phases](measurements/2026-10-25-labels-and-event-phases/readme.md).
- [Source-backed reference keys and separator delivery](measurements/2026-10-25-event-delivery/readme.md).

- [Unicode identifier folding and real documents](measurements/2026-10-25-normalize-identifiers/readme.md).
- [Inline destination scanning on real documents](measurements/2026-10-25-inline-destinations/readme.md).

- [Parser ownership and event-delivery consolidation](measurements/2026-10-25-parser-consolidation/readme.md).

- [Markdown serializer and AST edit consolidation](measurements/2026-10-25-serializer-and-edits/readme.md).
- [Static separators, borrowed unwrapping, and section-range removal](measurements/2026-10-25-static-separators-and-removal/readme.md).

- [Serializer scratch rewrites, marker scanning, and literal lookahead](measurements/2026-10-25-serializer-rewrites-and-lookahead/readme.md).

- [Footnote output, definition indexes, and borrowed escape context](measurements/2026-10-25-footnotes-indexes-and-escape-context/readme.md).
