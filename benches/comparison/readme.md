# Markdown parser comparison

This harness compares the public Markdown-to-HTML APIs of core-markdown, markdown-rs, Comrak, Markly, Commonmarker, and Inkmark. It lives outside the main workspace so competitor dependencies do not affect the published crate or ordinary tests. The adjacent [Rust](Cargo.lock) and [Ruby](Gemfile.lock) lockfiles pin the comparison dependencies.

## Recorded comparison

The [reference-label and event-phase study](measurements/2026-10-25-labels-and-event-phases/readme.md) compares production source **`f237acad0fa55ccd14477c0710244c3fcc443226`** with merged #50 and Inkmark 0.2.1. These numbers describe that recorded revision; subsequent source changes require new measurements. [Metadata](measurements/2026-10-25-labels-and-event-phases/metadata.json) records executable hashes, source trees, compiler, host, and lockfiles.

The original four workloads, in milliseconds per complete conversion; lower is faster:

| Workload | Input bytes | core-markdown ms | Inkmark ms |
| --- | ---: | ---: | ---: |
| README | 9,019 | 0.013592 | 0.018943 |
| Mixed 100 KB | 100,252 | 0.397763 | 0.409984 |
| Mixed 1 MB | 1,000,020 | 3.950691 | 4.349333 |
| GFM 100 KB | 100,130 | 0.609922 | 0.704936 |

Compared with #50, the README and mixed examples take 0.8–1.3% less time, while GFM takes 0.3% more. The largest definition workload improves by 1.4%; the smaller ones are essentially unchanged. Definition-heavy samples still take **2–8% more time than Inkmark**, and long-line, code, and multiline controls retain small regressions against #50. Separate prepare-plus-event-consumption diagnostics show no broad improvement (−0.2% to +1.9%). See the [full table and raw samples](measurements/2026-10-25-labels-and-event-phases/readme.md#direct-html) before generalizing from the original four.

These are local measurements on a shared Apple M4 Pro Mac, without CPU affinity or full machine isolation. Small differences are sensitive to background activity and process/compiler layout. **GFM has unequal output:** Inkmark leaves the workload's `www.example.com` unlinked and formats tables differently. The three original CommonMark samples agree with Inkmark after entity decoding, rather than byte for byte. These timings do not establish a universal ranking.

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

To reproduce a saved study, use its recorded source revisions, toolchain, lockfiles, and measurement driver. Keep immutable executables outside Cargo target directories, alternate engine order across rounds, and keep builds, tests, profiling, and allocation diagnostics outside the timing window. The [recorded study instructions](measurements/2026-10-25-labels-and-event-phases/readme.md#reproduction) document its three-round run; the quick command above performs one process run per engine/profile.

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

Byte differences can represent equivalent HTML escaping or formatting, or substantive differences in links, emphasis, tables, tag filtering, and footnotes. Read exact and normalized comparisons separately. The latest study preserves all 8,468 saved HTML/Markdown outputs and 24,276 diagnostic results against #50; its [output agreement](measurements/2026-10-25-labels-and-event-phases/output-agreement.json) and [preservation records](measurements/2026-10-25-labels-and-event-phases/preservation.json) identify the checked inputs and hashes. The [single-parser study](measurements/2026-10-25-single-parser/readme.md#output-and-compatibility) separately records intentional corrections outside the saved corpus. Observed agreement is regression evidence, not a proof for every input.

## Architecture and allocations

One source-backed parser handles CommonMark and independently enabled GFM, math, and frontmatter constructs. Block preparation records structure and definitions; a reusable inline scanner resolves each region when its events are pulled. Direct HTML consumes those events without constructing an AST. `Document` is the sole AST and builds incrementally from the same contract; its event traversal feeds the shared HTML writer. Ordinary values retain source ranges, while discontinuous or normalized values materialize only when needed. See the [pull architecture](../../context/pull-parser.md) and [document API](../../context/document.md).

The implementation retains `no_std`, shared byte scanners with AArch64 NEON and portable scalar paths, and reusable parser/output buffers. It has no grammar compatibility backend or separate mdast renderer; MDX support has been removed. Input is a complete UTF-8 buffer: pulling events does not imply chunked input or incremental source reparsing.

The recorded source has the same allocation traffic and event/node counts as #50 on the eight previously measured workloads. Its mixed 1 MB direct conversion makes 12 allocations and 27 reallocations; preparation makes 3 allocations and 14 reallocations, and pulling after preparation makes 7 allocations and 3 reallocations. Traversing a prepared document allocates nothing; document nodes measure 120 bytes on the recorded arm64 compiler. GFM and multiline normalization still allocate: the parser is not heap-free. [Complete allocation records](measurements/2026-10-25-labels-and-event-phases/allocations.json) include all measured operations.

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
