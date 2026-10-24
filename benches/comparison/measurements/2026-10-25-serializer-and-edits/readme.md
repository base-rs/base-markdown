# Markdown serializer and AST edit consolidation

Four implementation commits share destination/title formatting, move nine marker-validation modules beside their options, share arena-link operations, and share preorder advancement between traversal and edit refreshes. The net production diff is **125 fewer lines** (279 added, 404 removed), excluding the relocated unit-test file. Public APIs, feature support, semantic values and diagnostics remain unchanged.

The largest copy workload drops from **18.729 ms to 0.263 ms (71.2× faster)**. Other section-copy workloads are 6.5–20.1× faster; the three subtree-move workloads are 2.0–2.4× faster. HTML conversion and prepared-document Markdown serialization remain within approximately 1.2% of baseline. These small ordinary-workload differences do not establish a general speedup on this shared host. Inkmark was not retimed.

## Changes

1. **One destination/title writer:** definition and inline link/image handlers share delimiter emission, escaping scopes and title formatting. Callers retain their different empty-destination policies and the original quote-validation position.
2. **Construct-specific option checks:** private methods beside `MarkdownOptions` replace nine utility modules and share diagnostic construction. Checks stay at their existing call sites, including validation of a list item's fallback bullet even for ordered lists. Unused invalid options remain accepted, and exact messages, rule/source identifiers and error precedence are preserved.
3. **Shared arena linking:** insertion and movement use the same private link operations after public validation. Moves unlink/relink first and refresh subtree context once at the final destination. Section copying carries the previous insertion position, copies validated/materialized values directly, and refreshes each completed subtree. Corresponding source/destination parent links replace the temporary mapping stack. No persistent tail cache or additional node fields are introduced.
4. **Shared preorder successor:** `Descendants` and edit-context refresh use the same subtree-bounded successor helper. Refresh chooses the successor before mutating rendering hints. Event traversal keeps its existing enter/exit semantics.

The AST edit change adds 33 net production lines to make the link operations and operation-level refresh explicit; serializer/configuration consolidation removes 158. The parser, event contract, HTML writer, SIMD scanners, dependencies and public node layout are unchanged.

## Timing

Baseline is squash-merged #55, **`bd87ed19c23bfdc6e7187e4c7ab5c203902ada44`**, source tree **`8e564055055e70205d51bf2e3e6041ed159941c5`**. Candidate is **`a4754cb273af915288c06d7bd6492f5a54680812`**, production source tree **`4b2957eb7d2cca40beb976ed4604ac229806d506`**. That source tree was independently approved at `431933b`; the later candidate commit adds a public diagnostic regression test.

Milliseconds per operation; lower is faster. The [raw timing](timing.jsonl) retains all **24 process/mode records and 96 document/process results**. [The summary](timing-summary.json) retains every process median and spread; no samples were excluded.

| Operation/workload | #55 ms | Candidate ms | Time change |
| --- | ---: | ---: | ---: |
| html/readme | 0.013181 | 0.013252 | +0.54% |
| html/mixed-100k | 0.393614 | 0.390759 | -0.73% |
| html/mixed-1m | 3.915416 | 3.901881 | -0.35% |
| html/gfm-100k | 0.613864 | 0.606545 | -1.19% |
| markdown/readme | 0.086924 | 0.086844 | -0.09% |
| markdown/mixed-100k | 2.200705 | 2.201022 | +0.01% |
| markdown/mixed-1m | 22.006089 | 21.924292 | -0.37% |
| markdown/gfm-100k | 2.384973 | 2.357465 | -1.15% |
| replace/flat-64 | 0.188067 | 0.018464 | -90.18% |
| replace/flat-256 | 1.384847 | 0.068801 | -95.03% |
| replace/flat-1024 | 18.728531 | 0.263021 | -98.60% |
| replace/nested-32 | 0.207954 | 0.031942 | -84.64% |
| replace/nested-128 | 1.467129 | 0.126970 | -91.35% |
| move/list-64 | 0.008493 | 0.004279 | -49.62% |
| move/list-256 | 0.033106 | 0.013889 | -58.05% |
| move/list-1024 | 0.160531 | 0.074438 | -53.63% |

HTML includes complete parsing, rendering and output disposal on the established original four inputs. Markdown measures serialization and output disposal of already-parsed documents, so it is a different operation from earlier parse-plus-serialization measurements.

Section replacement includes cloning a prepared target, copying/materializing the replacement, editing, and dropping the result. Target parsing, replacement parsing and heading selection are outside timing. Each target retains 64 prefix paragraphs and a later heading. Flat replacements contain 64/256/1,024 paragraphs with emphasis and titled links; nested replacements contain 32/128 groups of quotes, nested quotes, lists and nested lists. Each operation starts from a fresh clone so detached nodes do not accumulate between iterations. The increasing benefit on flat copies is consistent with removing repeated sibling scans; every replacement source remains available throughout the measured operation.

A move operation moves the same list from one quote to another and back, including public validation and rendering-context refresh. The list contains 64/256/1,024 emphasized items. Prepared documents and their storage are reused; no document clone or rendering is included. This is the cost of a **pair** of moves.

Three alternating sequential baseline/candidate rounds run after this task's builds, tests, diagnostics and source review. Each document warms for at least 200 ms, followed by seven batches calibrated to approximately 150 ms. Tables use the median of three process medians. Every process uses a fresh executable inode. Input I/O, JSON and initial options are excluded. Timing executables use the native allocator without allocation instrumentation.

Host: Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. The machine is shared, without affinity or isolation; an unrelated Fantail observer uses roughly one core, with Yugabyte/macOS activity also present. No unrelated processes were stopped. Small effects on HTML/serialization warrant caution; the large editing changes recur in all three process runs. [Metadata](metadata.json) pins compilers, locks, source trees, input hashes, generated document sizes and frozen executable hashes.

## Allocation and output preservation

[Separate allocation diagnostics](allocations.json) record one operation after caller-owned reusable storage has warmed. Input/options setup and document parsing are excluded except for complete HTML conversion. Counts and cumulative requested bytes are unchanged for all four HTML conversions, all four serializations and all three move cases; moves allocate nothing. Result lengths/node counts agree in every diagnostic.

| Section copy | Fewer allocations | Fewer reallocations | Fewer requested bytes |
| --- | ---: | ---: | ---: |
| flat-64 | 127 | 0 | 8,128 |
| flat-256 | 511 | 0 | 32,704 |
| flat-1024 | 2,047 | 0 | 131,008 |
| nested-32 | 127 | 64 | 16,320 |
| nested-128 | 511 | 256 | 65,472 |

The old copy routine allocated a mapping vector even for each leaf/separator subtree. Removing that vector accounts for these allocation reductions. Requested bytes are cumulative allocation/growth requests, not peak or retained memory. Both versions still allocate arena storage and materialized values when copying across source buffers.

[Preservation results](preservation.json) record **8,468 unchanged corpus HTML/Markdown outputs**, **58,596 unchanged diagnostic results**, all four timed documents through direct/checked/document HTML and Markdown serialization, and **120 identical complete edited-arena snapshots**. Diagnostic results compare rendered HTML and Markdown values/errors, not complete serialized event sequences. Edited snapshots include detached original nodes, source/value representations as serialized, and syntax overrides; each candidate/baseline snapshot also deserializes successfully and retains its HTML. Serialization does not distinguish `Text::Literal` from borrowed/owned `Text::Value` storage.

The generated edit controls vary source shape, nesting, siblings, retained prefixes, and insertion at the document beginning/end. Public regression tests additionally exercise nested inline/container siblings, empty containers, copied-node provenance, metadata/footnotes/frontmatter, section flags, invalid edits, same-parent moves and exact/lazy option validation.

## Validation and reproduction

Native all-feature coverage passes **13,523/13,523 measured source regions** with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Linux x86_64 all-feature library compilation passes as a **compile-only** local check; native runtime coverage is AArch64. [Validation metadata](validation.json) records source identity, commands and log hashes. The complete evidence head is independently reviewed before opening the PR.

Use the [comparison setup](../../readme.md) and original fixtures. Generated maintenance fixtures are intentionally ignored and deterministically regenerated by [prepare.py](prepare.py); [metadata](metadata.json) retains their exact hashes. The original README input comes from the established comparison fixture rather than the changing current README. Corpus attribution/licenses remain in the linked original studies; this pass adds only small generated structural controls.

From the repository root, prepare original fixtures if absent, then use checkouts at the pinned baseline and candidate sources:

```sh
python3 benches/comparison/prepare.py
python3 benches/comparison/measurements/2026-10-25-serializer-and-edits/prepare.py
python3 benches/comparison/measurements/2026-10-25-serializer-and-edits/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-serializer-and-edits/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-serializer-and-edits/measure.py \
  --work /tmp/study --output /tmp/study/timing.jsonl
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/timing.jsonl
```

`build.py` uses the existing committed diagnostic lockfile and probe driver, and each source checkout's comparison lockfile. The diagnostic/timing maintenance adapter enables `serde`; only the separate allocation binary enables its counting allocator. All frozen executables live outside Cargo target directories. Use a fresh output path for a new timing cohort: the measurement driver resumes existing records and rejects changed input/binary identities. Verification requires the preserved official/custom fixtures prepared by the comparison setup, and reads archived probes from prior studies.
