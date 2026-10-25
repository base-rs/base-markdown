# Serializer scopes, frontmatter validation, and heading anchors

This study compares squash-merged #59 with four cleanup changes: remove unused serializer scope markers, share frontmatter fence checks, consolidate heading anchor storage, and remove obsolete hidden entity-decoding exports. Inkmark was not retimed.

## Implementation

- Remove twelve serializer markers that no escape rule consults, together with their push/pop calls. Retain all effective escape scopes and the blockquote/list/item markers used for nested-list disambiguation. This reduces scope folding work without adding state or caching.
- Private `CodeFence` helpers share opening-marker, info-string, and closing-delimiter checks between frontmatter recognition and AST syntax inference; serialization uses the same info-string predicate. Source-specific indentation remains with each caller.
- One heading map reserves both authored/generated anchors and stores the next suffix. Unique names need one retained key; generated names are reserved immediately, so later authored names cannot reuse them. Duplicate and empty-name collision behavior is preserved.
- Remove hidden public `decode_named` and `decode_numeric` and the unused 252-entry HTML 4 table. **This is a breaking API removal.** Markdown continues to use the unchanged HTML 5 table and the same numeric decoding. The hidden URL `sanitize` export remains.

The production diff removes **477 net lines** (82 inserted, 559 deleted), including documentation and entity-table data; the HTML 4 table accounts for 252 data rows. There are no dependency or feature changes. Successful output and diagnostics are unchanged.

## Source identities and results

Baseline is **`1e52f1a7a468fbb9bacd6b8d5c6671c993677535`** (#59), source tree `d54347706f7d6ef3ae4edf816293abc8ecd774ea`. Final production commit is **`daeb8762d4baa725a0cd723031c69e1f2757719d`**, source tree `021efa886d6357e7d8178022373b3b76d06fda09`. Separate commits cover scopes (`8255bfc`), frontmatter (`bc99cc9`), anchors (`2a01194`), and entity helpers (`7392e17`); `daeb876` removes an unused import. Independent production and harness reviews found no issues before timing. [Metadata](metadata.json) pins binaries, sources, inputs, drivers, compiler and locks.

Original complete HTML ranges from **1.4% less to 0.7% more time**; prepared-document Markdown ranges from **0.5% less to 0.8% more time**. These small changes overlap observed process variation; this study establishes no ordinary-document speedup. Heading controls are the material gain: unique-name HTML takes **32.4% less time**, and prepared heading enumeration **40.2% less time**. Duplicate/suffix-collision controls take **1.8–7.1% less time**. Frontmatter consolidation leaves the 100 KB controls within 0.3%; smaller controls take 1.3–3.7% less time, with variation shown below.

All timings are milliseconds; lower is faster. Time change is relative to baseline. Process spread is `(maximum / minimum - 1) × 100` across three process medians, not a confidence interval.

### Original four workloads

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.013230 | 0.013301 | +0.53% | 1.0% / 1.0% |
| html/mixed-100k | 0.392567 | 0.395217 | +0.67% | 1.2% / 3.1% |
| html/mixed-1m | 3.947297 | 3.971831 | +0.62% | 1.9% / 1.5% |
| html/gfm-100k | 0.621295 | 0.612691 | -1.38% | 7.6% / 0.3% |
| markdown/readme | 0.063200 | 0.063037 | -0.26% | 1.1% / 1.0% |
| markdown/mixed-100k | 1.387773 | 1.399459 | +0.84% | 1.7% / 2.1% |
| markdown/mixed-1m | 13.928587 | 13.863598 | -0.47% | 2.6% / 0.8% |
| markdown/gfm-100k | 1.593813 | 1.594832 | +0.06% | 2.1% / 1.4% |

### Targeted controls

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| features/html-unique-512 | 0.239558 | 0.161975 | -32.39% | 3.6% / 0.9% |
| features/headings-unique-512 | 0.195508 | 0.116995 | -40.16% | 1.7% / 1.0% |
| features/html-repeated-512 | 0.165768 | 0.162798 | -1.79% | 2.2% / 2.2% |
| features/headings-repeated-512 | 0.126097 | 0.120958 | -4.08% | 1.8% / 2.0% |
| features/html-suffixes-512 | 0.187577 | 0.175706 | -6.33% | 1.2% / 1.3% |
| features/headings-suffixes-512 | 0.144695 | 0.134382 | -7.13% | 3.7% / 1.8% |
| features/frontmatter-dash-32 | 0.000264 | 0.000261 | -1.34% | 4.2% / 2.2% |
| features/frontmatter-dash-100000 | 0.002086 | 0.002085 | -0.02% | 1.1% / 0.1% |
| features/frontmatter-tilde-32 | 0.000268 | 0.000258 | -3.69% | 1.7% / 2.8% |
| features/frontmatter-tilde-100000 | 0.002102 | 0.002100 | -0.10% | 0.7% / 1.0% |
| features/frontmatter-backtick-32 | 0.000269 | 0.000261 | -2.74% | 0.8% / 1.0% |
| features/frontmatter-backtick-100000 | 0.002107 | 0.002100 | -0.30% | 0.4% / 0.8% |

Original HTML and targeted HTML include parsing, rendering and output disposal. Original Markdown uses prepared documents, including output disposal but excluding parsing. Heading enumeration uses prepared documents and includes disposing the returned vector and strings. Setup/options and parsing for prepared-document operations stay outside timing. Heading controls enable heading IDs for HTML; ordinary original workloads do not. Each heading control contains 512 names. Frontmatter controls enable frontmatter, include trailing prose, and cover dash, tilde and longer backtick closing fences at two sizes.

[Primary samples](primary.jsonl) and [summary](primary-summary.json), and [targeted samples](targeted.jsonl) and [summary](targeted-summary.json), retain every completed batch and process. There are 18 process/mode records containing 120 document/process results; no samples were discarded.

## Allocations and output preservation

[Allocation records](allocations.json) count one warmed operation with a separate instrumented executable. Timing uses the native allocator. All 52 baseline/candidate result lengths or node counts match. Counts exclude caller setup; complete HTML includes parsing. Requested bytes are cumulative allocation and growth requests, not peak or retained memory.

Unique heading HTML and enumeration each avoid **591 allocations** and request **28,698 fewer bytes** per 512 headings. Repeated headings avoid **513 allocations** but request **4,624 more cumulative bytes** because map entries carry suffix values for generated anchors. Suffix-collision controls avoid **404 allocations** and request **1,830 fewer bytes**. The map consolidation therefore reduces allocation count on these controls, but is not a universal reduction in requested bytes.

The synthetic 100 KB math-flow serialization control avoids one 8-byte allocation after removing its unused scope marker. Other measured original, serializer, edit, footnote/reference and frontmatter allocation counts are unchanged.

[Preservation results](preservation.json) compare **8,468 corpus outputs**, **58,596 diagnostic probes**, original direct/checked/document HTML and Markdown outputs, and **120 complete edited-arena snapshots**. An additional **25,543 complete records** preserve serializer output/errors, direct/document HTML parity, inferred syntax and heading text/anchors as appropriate: 23,082 existing serializer probes, 2,425 new heading/frontmatter probes and 36 controls/features. New probes enumerate authored/generated heading-name collisions and frontmatter markers, lengths, opaque info, CR/LF/CRLF, indentation and closing lengths. A public regression adds empty names and authored/generated suffix collisions. Diagnostics are compared as returned results, not complete event sequences.

## Validation and reproduction

Apple M4 Pro, AArch64 macOS; timing uses Homebrew Rust 1.99.1/LLVM 23.1.3. Three sequential alternating rounds reverse engine order in round two, launching fresh executable inodes. Each document warms for at least 200 ms and then runs seven batches calibrated to approximately 150 ms. Tables report medians of three process medians. Startup, JSON and setup are outside timing. Own builds, tests, output/allocation diagnostics and review did not overlap timing. The host is shared without affinity/isolation, and unrelated processes remained running; small effects do not establish a universal performance ranking.

Native all-feature/all-target coverage passes **13,712/13,712 measured source regions**, with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy, formatting, and Linux x86_64 all-feature library compilation pass. The local Linux check is compile-only; CI runs Linux runtime tests and coverage. [Validation](validation.json) pins source identities and check-log hashes.

The [generator](prepare.py), [builder](build.py), [adapter](bench.rs), [verifier](verify.py), and [recorder](record.py) reuse immutable historical calibration, counting allocator, corpus fixtures and drivers. Their source identities are pinned in metadata. Generated fixtures are ignored and hashed. Corpus attribution/licenses remain in the preceding studies. Freeze binaries outside Cargo target directories and retain them until all output and timing work is complete.

```sh
python3 benches/comparison/measurements/2026-10-25-scopes-frontmatter-and-anchors/prepare.py
python3 benches/comparison/measurements/2026-10-25-scopes-frontmatter-and-anchors/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-scopes-frontmatter-and-anchors/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/primary.jsonl --mode html --mode markdown
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/targeted.jsonl --mode features \
  --input features=benches/comparison/measurements/2026-10-25-scopes-frontmatter-and-anchors/targeted.json
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/primary.jsonl --input /tmp/study/targeted.jsonl
```

Use the recorded source revisions, lockfiles and compiler when reproducing the saved results. The driver validates existing input/binary hashes when resuming and retains all results.
