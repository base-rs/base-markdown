# A single source-backed AST

This change replaces the recursive mdast AST, its compiler, and its separate HTML renderer with the existing `Document` arena. It removes **5,289 net lines under `src/`** while migrating useful editing, heading, frontmatter, Markdown serialization, and serde capabilities. The [migration guide](../../../../context/document.md) describes the deliberate breaking API and serialized-format changes.

The semantic pull event stream remains the central contract. Direct HTML consumes parser events; the AST builder consumes them incrementally; document traversal emits the same events. Custom renderers can consume `EventParser`, `Document::events`, or subtree events, without a second AST hierarchy. The built-in HTML writer has no per-tag override hook API.

## Source and method

Baseline is merged main `84bd53b4b65c35e13cb3821359900936bd088d4d`. Its immutable executable was built at `5f3f83e86896955ffa1360a0aa87af9eda186fba`, whose production source tree is identical to that main commit (`8efeb7b09f2309eff7e42834c679c6ab788bec6f`). Final executable was built at `c2b642ece700c44272ac7c4c6134286a213ec4d6`, source tree `00589535cfaa5296b4dacf60039345f665531513`. Later test-only lint fixes and evidence commits leave that source unchanged. [Metadata](metadata.json) records compiler, machine, build command, and executable hashes.

Measurements ran on the local Apple M4 Pro, arm64 macOS, Rust 1.99.1, Ruby 4.0.9 and Inkmark 0.2.1. Each table shows the median of three process medians, each using seven calibrated samples after 200 ms warmup. Engine order alternates; binaries use a common launch path with a fresh inode. Parsing, rendering, disposal and ordinary Ruby GC are included; startup, input loading and initial option construction are excluded. No builds, tests, profiling, or review overlapped timing. This is a shared machine with background processes and no CPU affinity; the [CPU snapshot](metadata.json) records substantial background activity. Small differences are not evidence of a reliable speedup.

## Direct HTML

Milliseconds per complete conversion; lower is faster. [Every raw result](html.jsonl) and the [recomputed summary](html-summary.json) retain all samples and process ranges.

| Workload | Main ms | Document branch ms | Inkmark ms | Time reduction vs main |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 0.01361 | 0.01356 | 0.01890 | 0.4% |
| Mixed 100 KB | 0.39612 | 0.39346 | 0.40629 | 0.7% |
| Mixed 1 MB | 3.99278 | 3.95152 | 4.37519 | 1.0% |
| GFM 100 KB | 0.59333 | 0.58984 | 0.70608 | 0.6% |
| References 80 KB | 0.53703 | 0.53577 | 0.85932 | 0.2% |
| Long prose 1 MB | 0.11315 | 0.11299 | 0.57970 | 0.1% |
| Fenced code 1 MB | 0.06995 | 0.07011 | 0.31542 | -0.2% |
| Entities 100 KB | 0.81044 | 0.80181 | 0.94878 | 1.1% |
| Ordinary HTML 100 KB | 0.25408 | 0.25659 | 0.32089 | -1.0% |
| Filtered HTML 100 KB | 0.27427 | 0.27830 | 0.29421 | -1.5% |
| Definitions 256 | 0.04101 | 0.04063 | 0.04328 | 0.9% |
| Definitions 1,024 | 0.16811 | 0.16810 | 0.16745 | 0.0% |
| Definitions 4,096 | 0.73469 | 0.74398 | 0.69633 | -1.3% |

Direct HTML is effectively flat: the original four take 0.4–1.0% less time, while some diagnostics take up to 1.5% more. This pass establishes substantial simplification without a material direct-render regression, rather than a new fast-path optimization. Inkmark is slower on the original four medians here, but faster on the two larger definition-heavy cases. This is not a universal parser ranking.

**GFM is an unequal-output comparison.** Inkmark leaves the fixture's `www.` domain unlinked and differs in table formatting. CommonMark README/mixed outputs agree after entity decoding, not byte-for-byte. The [output agreement records](benchmark-output-agreement.json) include each fixture's hashes. No global conformance claim follows from these timings.

## AST conversion

`core-ast` in the baseline parses into mdast and uses its legacy HTML renderer; in the final executable it parses into `Document` and uses the shared event HTML writer. These are complete parse-plus-render operations, including disposal. They do not isolate rendering cost. The types and serialized AST schema deliberately change. Legacy HTML is not byte-identical to the canonical event writer: these fixtures differ in terminal/nested-list whitespace, table formatting, and task markup. [AST output hashes](ast-output-agreement.json) retain that distinction; preservation claims below apply to the existing event/arena output.

| Workload | Legacy AST + HTML ms | Document + HTML ms | Time reduction |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.14058 | 0.02766 | 80.3% |
| Mixed 100 KB | 4.39209 | 0.84291 | 80.8% |
| Mixed 1 MB | 49.93629 | 9.27640 | 81.4% |
| GFM 100 KB | 6.84139 | 1.28293 | 81.2% |

This is about **5.1–5.4× the legacy path's throughput** on these fixtures. [Raw AST timings](ast.jsonl) and [summary](ast-summary.json) retain the evidence. A separate matched arena comparison checks the cost of the new helper/syntax state against the arena already on main:

| Workload | Main arena + HTML ms | Final arena + HTML ms |
| --- | ---: | ---: |
| README 9 KB | 0.02766 | 0.02765 |
| Mixed 100 KB | 0.85451 | 0.84409 |
| Mixed 1 MB | 9.29209 | 9.24665 |
| GFM 100 KB | 1.29546 | 1.28870 |

Arena results remain within about 1.2%, with overlapping process ranges. [Raw arena timings](arena.jsonl) and [summary](arena-summary.json) show no meaningful added throughput cost. The metadata map is empty and allocation-free for ordinary parsing; authored syntax is inferred on demand or stored sparsely for edits. No event queue or full-stream intermediate is added.

## Allocation traffic and limits

[Allocation records](allocations.json) use a separate counting executable, excluding input loading and initial options. All thirteen fixtures retain exactly the same direct-HTML and arena-construction allocation counts, requested bytes, node counts and traversal allocation counts as main. Traversal allocates nothing. The [layout probe](layout.json) confirms unchanged arm64 sizes: `EventParser` 1,104 bytes, `DocumentNode` 120 bytes, `AstBuilder` 64 bytes.

| AST construction | Legacy allocation calls / reallocations | Document allocation calls / reallocations | Legacy requested bytes | Document requested bytes |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 1,361 / 149 | 17 / 18 | 1,262,588 | 151,945 |
| Mixed 100 KB | 36,698 / 4,057 | 12 / 27 | 49,298,185 | 4,598,464 |
| Mixed 1 MB | 364,576 / 38,749 | 12 / 33 | 475,710,621 | 37,978,776 |
| GFM 100 KB | 50,366 / 5,207 | 371 / 386 | 70,623,291 | 9,217,311 |

Requested bytes are cumulative allocation/growth traffic, not peak or retained memory. Arena nodes store source ranges instead of materializing each value. Edited values and cross-document copies materialize only what they need; detached slots remain until document drop.

Markdown formatting still uses the retained serializer's regex/escaping machinery and is allocation-heavy. The final 1 MB mixed document's serialization alone records 20,017,734 allocation calls and about 5.36 GB of cumulative requests. This report does not establish a Markdown-serialization speedup or a regression against the old serializer; it is an explicit remaining optimization target. The AST and direct-HTML results must not be extended to that operation.

## Validation and reproduction

All **2,117 saved outputs** remain byte-identical to main through direct HTML, checked-event rendering, and arena traversal. [Preservation records](preservation.json) retain the corpus hashes and path counts. Native all-feature/all-target coverage passes **21,278/21,278 measured source regions**, excluding 37 invariant-only unreachable regions. Default full workspace tests, all-feature and default all-target Clippy, formatting, and Linux all-feature library cross-compilation pass. [Validation records](validation.json) distinguish native runtime checks from Linux compile-only checks. Independent implementation review found and resolved escaped-footnote-label, copied-reference-collision, and heading-helper parity defects before timing.

Build baseline and final adapters separately with the pinned comparison lockfile, preserve immutable executables outside Cargo's target directory, and use [the comparison setup](../../readme.md). Generate entity/HTML inputs with the preceding study's `fixtures.py WORK/data`. The local evidence drivers retain `/tmp` executable/gem paths; substitute equivalent local paths. For example:

```sh
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary final=/tmp/study/final --binary inkmark=inkmark \
  --rounds 3 --suite html --work /tmp/study --output html.jsonl
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary final=/tmp/study/final --rounds 3 --suite ast \
  --work /tmp/study --output ast.jsonl
python3 summarize.py
```

The same driver supports `--suite arena`. `summarize.py` recomputes every sample/process median; no slow samples are discarded. `output-agreement.py` records final-vs-Inkmark agreement. The preceding study's `verify.py` and `allocations.py` accept the immutable binary paths for corpus preservation and allocation reproduction.
