# List serialization and grammar reference cleanup

This study compares merged PR #46 (`bb6f198`) with the source identified in
[metadata.json](metadata.json). The goal is to remove redundant work and retained
state while preserving the semantic event contract and ordinary conversion speed.

## Implementation

- Ordered-list serialization reuses the existing traversal ordinal instead of
  scanning siblings for every item. It writes the number, bullet, and task prefix
  into the shared output rather than allocating temporary marker strings.
- Grammar inline tokenization borrows the completed semantic definition index and
  footnote identifiers. Block preparation no longer separately normalizes,
  collects, and propagates regular reference keys. The old private eager-token
  test helper and its unused completion bookkeeping are removed; internal tests
  initialize their required context directly or exercise the production producer.
- Explicit document/inline phases replace two lifecycle booleans. Inline promotion
  skips document line-ending detection, and both producers share one byte scanner
  for that metadata.

The grammar event reader and reusable buffers remain shared. Splitting that reader
into separate block/inline implementations would add ownership and dispatch
complexity to remove some empty vector headers. This change takes the smaller
ownership/lifecycle cleanup instead; it does not implement that larger split.

Direct serialization of an attached ordered item also fixes an existing bug: an
item numbered ten now uses `10.` (or the configured ordered marker), replacing
the previous invalid `10-`. Whole-document saved outputs are unchanged.

## Method

Native release binaries use Homebrew Rust 1.99.1 on Apple M4 Pro, arm64 macOS.
Inkmark is the installed 0.2.1 Ruby gem, Ruby 4.0.9. Validation uses Rustup stable.
Each table reports the median of three process medians, each containing seven
calibrated samples after 200 ms warmup. Engine order alternates. Native adapters
launch fresh executable inodes at the same path. Startup, input loading, and
initial option construction are outside the measured batches; conversion,
disposal, and ordinary Ruby GC are included.

The reported matrix monitors competing compiler/Cargo processes every 0.5 seconds,
waits before starting a conversion, and terminates/retries it if those drivers
appear. Local source review, profiling, and allocation diagnostics are idle during
timing. An independent project's coverage run started during the first attempt;
that entire unmonitored attempt is excluded from the tables and retained in
[interrupted-attempt](interrupted-attempt/reason.json). This remains a shared machine without CPU affinity: persistent background
workloads and process variation limit conclusions about small percentage changes.
[The starting process snapshot](timing-background-start.json) records that load.
Every completed sample and process median is retained, including slow samples.
The monitor does not establish an otherwise idle machine or detect all possible
background workloads, and short activity between polls may go undetected.

## Original direct HTML workloads

Times are milliseconds; lower is better. `baseline` is merged PR #46, and `final`
is this cleanup. [Raw samples](html.jsonl), [summary](html-summary.json).

| Workload | baseline | final | inkmark |
| --- | ---: | ---: | ---: |
| readme | 0.013592 | 0.013503 | 0.018975 |
| mixed-100k | 0.392194 | 0.393565 | 0.410058 |
| mixed-1m | 3.930108 | 3.921702 | 4.407571 |
| gfm-100k | 0.592437 | 0.593964 | 0.713757 |

The four original HTML times change by **−0.7% to +0.4%** against baseline.
Current times are **28.8%, 4.0%, 11.0%, and 16.8% lower than Inkmark**, respectively.
GFM has the unequal-output limitation described below; the CommonMark README and
mixed samples agree with Inkmark after entity decoding, rather than byte-for-byte.
These small before/after differences do not establish a general HTML speedup.

Additional diagnostics:

| Workload | baseline | final | inkmark |
| --- | ---: | ---: | ---: |
| references | 0.543800 | 0.539420 | 0.849613 |
| long-lines-1m | 0.112162 | 0.109865 | 0.587449 |
| code-1m | 0.070235 | 0.070111 | 0.320574 |
| entities-100k | 0.803185 | 0.833787 | 0.953994 |
| html-safe-100k | 0.261073 | 0.261057 | 0.324699 |
| html-filtered-100k | 0.279293 | 0.280977 | 0.296586 |
| definitions-256 | 0.040701 | 0.040460 | 0.043087 |
| definitions-1024 | 0.168882 | 0.166226 | 0.168436 |
| definitions-4096 | 0.739736 | 0.733886 | 0.692465 |

The entity-heavy case takes **3.8% longer**. A separate alternating confirmation
also finds a **3.3% slowdown**: 0.812945 → 0.840108 ms. Its list-only intermediate
already takes 2.5% longer on this unchanged direct-HTML path. The cause is not
established; this report retains the regression rather than claiming universal
improvement. [Confirmation samples](entities.jsonl), [summary](entities-summary.json).
The 4,096-definition compact case remains about 6.0% slower than Inkmark.

## List scaling and Markdown conversion

These timings include parsing, arena construction, and Markdown serialization;
they are not prepared-AST-only timings. `lists` isolates the direct-marker and
traversal-position commit. [Scaling samples](lists.jsonl), [summary](lists-summary.json).

| Workload | baseline | lists | final |
| --- | ---: | ---: | ---: |
| ordered-list-256 | 0.163342 | 0.079302 | 0.081001 |
| ordered-list-2048 | 12.017074 | 0.624775 | 0.636829 |
| ordered-list-8192 | 246.646791 | 2.535122 | 2.582130 |

At 8,192 ordered items, final conversion has **95.5× baseline throughput** and
uses 98.95% less time. Final timings grow about eightfold from 256 to 2,048 items
and fourfold from 2,048 to 8,192, consistent with the linear traversal replacing
quadratic sibling rescans.

The original four inputs contain shorter lists and show modest conversion
changes: −1.0% to +0.2% time. [Samples](markdown.jsonl), [summary](markdown-summary.json).

| Workload | baseline | lists | final |
| --- | ---: | ---: | ---: |
| readme | 0.106670 | 0.106451 | 0.106891 |
| mixed-100k | 2.808027 | 2.788163 | 2.805861 |
| mixed-1m | 28.806882 | 28.603111 | 28.512674 |
| gfm-100k | 3.266432 | 3.256218 | 3.259875 |

## Grammar reference workloads

`grammar-definitions` puts all reference uses in a paragraph containing multiline
inline HTML, forcing inline grammar tokenization. `grammar-full` uses multiline
definition titles, forcing grammar block preparation. `grammar-reuse` alternates
compact paragraphs and multiple inline grammar regions with a forward reference.
[Fixture generator](fixtures.py), [samples](grammar.jsonl), [summary](grammar-summary.json).

`references` is the initial borrowed-lookup commit; it still collects redundant
regular keys during block preparation. Final removes that collection, as well as
sharing metadata scanning and clarifying lifecycles. Intermediate source mappings
and `pre-removal` measurements remain in metadata and raw records.

| Workload | baseline | references | final | inkmark |
| --- | ---: | ---: | ---: | ---: |
| grammar-definitions-256 | 0.213135 | 0.179388 | 0.172566 | 0.058588 |
| grammar-definitions-1024 | 1.525206 | 0.735594 | 0.718821 | 0.228503 |
| grammar-definitions-4096 | 13.912152 | 3.145944 | 3.060074 | 0.933269 |
| grammar-full-definitions-4096 | 8.894266 | 9.091257 | 8.385359 | 1.059440 |
| grammar-full-no-links-4096 | 7.639862 | 8.012333 | 7.528567 | 0.513205 |
| grammar-reuse-100k | 3.075468 | 3.054742 | 3.032058 | 1.038575 |

The 4,096-reference inline fallback has **4.55× baseline throughput**, reducing time
by 78.0%. Full grammar preparation improves less: 5.7% with links and 1.5% without
links; the repeated-region workload changes by 1.4%. These grammar stress cases
**remain slower than Inkmark**. Their output agreement is recorded separately;
multiline inline-HTML cases differ in continuation indentation. Full-grammar
definition cases are byte-equal to Inkmark.
The timing gains are comparisons against our own preserved baseline output.

## Document metadata scanning

These diagnostics isolate a long source with no line ending and a first newline
near the end, where multiline HTML requires inline fallback. `workspace` adds the
explicit lifecycle and avoids document metadata scanning during promotion;
`pre-removal` then shares byte-based line-ending detection. [Samples](line-ending.jsonl),
[summary](line-ending-summary.json).

| Workload | baseline | workspace | pre-removal | final |
| --- | ---: | ---: | ---: | ---: |
| no-line-ending-100k | 0.055864 | 0.055884 | 0.010654 | 0.010639 |
| late-inline-fallback-100k | 0.161085 | 0.113815 | 0.068874 | 0.068457 |

No-newline prose has **5.25× baseline throughput**. The late inline-fallback case
has **2.35× baseline throughput**: both avoiding the duplicate scan and making the
remaining scan byte-based contribute. This is a targeted benefit, not an estimate
for arbitrary documents. These timings do not include Inkmark; its no-newline
fixture output adds a final newline, and the multiline-HTML fixture differs in
continuation indentation.

## Allocation and layout checks

Allocation diagnostics run independently of timing. `ast_markdown` serializes a
prepared document, excluding parsing and AST construction.

| Workload / operation | Baseline allocation calls | Final allocation calls |
| --- | ---: | ---: |
| Mixed 1 MB / prepared Markdown serialization | 20,542 | 7,707 |
| 8,192 ordered items / prepared Markdown serialization | 24,580 | 8,196 |
| 4,096-reference inline fallback / direct HTML | 36,890 | 32,793 |
| Full grammar, 4,096 definitions with links / direct HTML | 53,306 | 49,205 |
| Full grammar, 4,096 definitions without links / direct HTML | 53,300 | 49,199 |

Mixed 1 MB serializer reallocations stay at 26; cumulative requested bytes fall
from 2,706,145 to 2,657,372. Original direct-HTML and Document allocation counters
are unchanged on all thirteen original diagnostics. Event counts, node counts,
and allocation-free arena traversal are preserved throughout the diagnostic set.
[Full counters](allocations.json).

The retained grammar workspace shrinks **1,024 → 952 bytes**. Borrowed ParseState
shrinks 72 → 48 bytes; it is created locally rather than retained in that workspace.
The public EventParser handle remains 368 bytes and AstBuilder remains 64 bytes.
These are native compiler layouts, not stable API/ABI guarantees. [Layouts](layout.json).

Arena construction plus HTML conversion remains close to baseline, with time
changes from −0.5% to +0.5%. [Samples](arena.jsonl), [summary](arena-summary.json).

| Workload | baseline | final |
| --- | ---: | ---: |
| readme | 0.027693 | 0.027602 |
| mixed-100k | 0.852279 | 0.850678 |
| mixed-1m | 9.411510 | 9.368975 |
| gfm-100k | 1.301562 | 1.308270 |

## Validation and output

[Validation records](validation.json) identify the frozen source, log hashes, and
independent review. Native all-feature/all-target coverage reaches **20,984/20,984
measured source regions**, with 38 invariant-only `unreachable!` exclusions.
Default workspace tests and both Clippy configurations pass. Linux is a local
library compile check; repository CI supplies Linux runtime coverage.

Exact preservation includes [8,468 saved corpus results](output-preservation.json),
[48,000 generated angle results](angles-preservation.json), and
[44 new diagnostic results](diagnostic-preservation.json), across direct HTML,
checked events, arena HTML, and Markdown conversion. The attached-item marker
correction is covered separately by a public regression.

[Agreement records](benchmark-output-agreement.json) retain input/output hashes
and both byte equality and equality after entity decoding against Inkmark. Entity
decoding is not a conformance proof. In particular, the original GFM sample has
unequal output: Inkmark leaves its `www.` domain unlinked and differs in table
formatting. These are workload comparisons, not a universal parser ranking.

## Reproduction

Preserve release timing and allocation executables for each source revision.
`metadata.json` maps all measured stages to commits, source trees, and executable
hashes; baseline binaries match the preceding study's final executables.

```sh
cargo build --manifest-path benches/comparison/Cargo.toml --release --locked --bins
python3 benches/comparison/measurements/2026-10-24-streamline-lists-and-references/fixtures.py "$WORK/data"
python3 benches/comparison/measurements/2026-10-24-simplify-events/fixtures.py "$WORK/data"
python3 benches/comparison/measurements/2026-10-24-streamline-lists-and-references/run-all.py --work "$WORK"
```

`$WORK` must contain the labeled immutable executables from the metadata and the
generated data. The main comparison inputs and Ruby dependencies use
`benches/comparison`'s existing pinned setup. The allocation script runs separately;
its counters measure cumulative allocation/growth requests, not peak memory.
`preserve.py`, `angles-preservation.py`, `diagnostic-preservation.py`, and
`output-agreement.py` reproduce preservation checks. Keep timing isolated from all
other validation workloads.
