# Markdown scratch rewrites, marker scanning, and literal lookahead

Four source commits consolidate Markdown serialization without changing public APIs, features, node layout, dependencies or output. The net production diff is 11 fewer lines, excluding tests, and removes the separate longest-character-streak module. Inkmark was not retimed in this study.

## Changes

1. **Shared scratch rewrites:** inline code/math normalization uses the same temporary-buffer management as tables and indentation. The private callback can mutate the detached scratch string. Values without CR/LF skip normalization and its output copy: every selected normalization rule requires a preceding physical line ending, and literal padding adds only spaces.
2. **Shared marker runs:** one memchr-based byte iterator yields complete runs for inline delimiter selection, code/math/frontmatter fence lengths and table backtick matching. Inline delimiters still choose the first absent run length; block fences still choose longest-plus-one with their existing minima. Table matching still requires an exact complete-run length. A shared continuation helper consumes a current run whose start is already known, avoiding a redundant search. The existing inline run-length vector/sort remains; the scanner is allocation-free, but delimiter selection is not universally allocation-free.
3. **Conservative literal lookahead:** text and soft-break values can expose their first character directly when it is outside every escape rule's candidate byte. The predicate uses the existing rules rather than another classification table. CR/LF, ASCII spaces/tabs and other candidate prefixes retain speculative rendering. Empty values and unsupported node shapes retain the old path, including diagnostic order. Skipped text rendering has no semantic state changes; output/escaping scratch is transient and rebuilt before use.

The parser, semantic event contract, direct HTML renderer and AST edit algorithms are unchanged. These changes target prepared-document Markdown serialization.

## Sources and timing

Baseline is squash-merged #57, **`225cc0b8704e1c53dfeb060b16f6cae7551dc2dc`**, source tree **`413a2dc9cabb8d410bd45f4195c4ed00a1a9e330`**. Incremental stages are `b668e4b` (scratch rewrites), `172c7cf` (shared marker runs), **`a141c028a6e92bfc0b28699d5417257e3ea9c32a`** (literal lookahead), and **`c88e46aafe672ec44f0c15f27fece7221981a87e`** (known-run continuation), final source tree **`fc6a3b57ae9ed031fdce806d10f760b1889d7c8b`**. The exact final source was independently approved before timing. [Metadata](metadata.json) pins full commits, source trees, compilers, locks, input/generator hashes, document sizes and all ten frozen executables, including the intermediate stages.

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.013362 | 0.013304 | -0.43% | 0.8% / 0.1% |
| html/mixed-100k | 0.393961 | 0.397164 | +0.81% | 0.7% / 0.3% |
| html/mixed-1m | 3.944354 | 4.014431 | +1.78% | 0.8% / 0.5% |
| html/gfm-100k | 0.614960 | 0.618647 | +0.60% | 0.5% / 1.6% |
| markdown/readme | 0.088674 | 0.082594 | -6.86% | 0.6% / 0.9% |
| markdown/mixed-100k | 2.238672 | 2.064717 | -7.77% | 1.8% / 0.2% |
| markdown/mixed-1m | 22.199250 | 20.351104 | -8.33% | 1.2% / 2.3% |
| markdown/gfm-100k | 2.390346 | 2.269924 | -5.04% | 2.2% / 0.2% |

Prepared-document Markdown serialization takes **5.0–8.3% less time** on the original four. Complete HTML ranges from **0.4% less to 1.8% more time**; mixed-1m is consistently slower across the three process medians, and the initial cohort also measured it 1.5% slower. The direct HTML source path is unchanged, but these results do not establish that its cost is identical. We retain that small regression rather than dismissing it as an anomaly.

Original HTML includes parsing, rendering and output disposal. Original Markdown serializes and drops output from a prepared document; parsing is outside timing. This differs from earlier parse-plus-serialization measurements.

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| controls/code-plain | 224.732709 | 224.337500 | -0.18% | 0.6% / 0.5% |
| controls/code-marker-runs | 697.037000 | 698.234125 | +0.17% | 0.3% / 15.8% |
| controls/code-multiline | 72.774028 | 72.767306 | -0.01% | 0.3% / 0.9% |
| controls/math-plain | 183.445833 | 183.831833 | +0.21% | 0.6% / 0.5% |
| controls/math-flow-100k | 0.038979 | 0.016786 | -56.94% | 0.0% / 0.8% |
| controls/fenced-code-1m | 0.477584 | 0.022379 | -95.31% | 2.6% / 1.0% |
| controls/frontmatter-100k | 0.044286 | 0.037677 | -14.92% | 0.2% / 0.7% |
| controls/table-code-256 | 0.043693 | 0.023184 | -46.94% | 6.8% / 1.2% |
| controls/adjacent-ascii | 80.534875 | 80.401958 | -0.17% | 0.2% / 0.5% |
| controls/adjacent-unicode | 116.558375 | 116.268146 | -0.25% | 0.3% / 0.5% |
| controls/adjacent-punctuation | 33.974908 | 34.015150 | +0.12% | 0.3% / 0.1% |
| controls/code-plain-8 | 0.005820 | 0.005387 | -7.43% | 1.0% / 1.3% |
| controls/math-plain-8 | 0.005039 | 0.004647 | -7.78% | 0.8% / 0.9% |
| controls/adjacent-ascii-8 | 0.003111 | 0.002120 | -31.84% | 3.8% / 2.2% |
| controls/adjacent-unicode-8 | 0.003846 | 0.002809 | -26.95% | 1.0% / 1.2% |
| controls/adjacent-punctuation-8 | 0.013241 | 0.013379 | +1.04% | 1.5% / 1.2% |

The large code fence takes **95.3% less time** (21.3× faster), math block **56.9% less**, frontmatter **14.9% less**, and table **46.9% less**. Short plain code/math take **7.4%/7.8% less time** and short adjacent ASCII/Unicode text **31.8%/27.0% less**. Large densely adjacent leaf controls remain within 0.3%; repeated escaping of growing preceding context dominates their work. Short punctuation-heavy text takes **1.0% more time**. The candidate code-marker control has a 15.8% process spread; its slow round is retained.

The final cohort compares baseline and final candidate on all 24 workloads: four HTML, four Markdown and sixteen serializer controls. It retains 18 process/mode records and 144 document/process results.

All controls measure serialization/output disposal of prepared documents. Code/plain, code/marker-runs and math/plain each contain 2,048 literal/text pairs. Code/multiline contains 1,024 synthetic code/text pairs with embedded CRLF/LF and block-opening syntax; parsing ordinary inline code would normalize its line endings before serialization. The 100 KB math block is synthetic; the approximately 1 MB code fence and 100 KB frontmatter controls are parsed from generated source. The table has 256 code-containing rows. Adjacent-text controls use 2,048 separate ASCII/Unicode text nodes or 512 triples beginning with punctuation/space. These custom adjacent nodes retain chunk boundaries that a parser could otherwise combine. Five additional short controls use eight repetitions each of plain code, plain math, ASCII text, Unicode text and punctuation-heavy text.

Each cohort runs three alternating sequential rounds, using fresh executable inodes. Each document warms for at least 200 ms, then runs seven batches calibrated to approximately 150 ms. Tables use medians of three process medians. Every completed raw sample is retained. Engines reverse order in the second round; incremental stages are independent complete operations, not additive timing phases. Input/options/document construction, process startup and JSON are excluded. Timing uses the native allocator, with allocation diagnostics run separately.

Host: Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. This task's builds, tests, diagnostics and source review did not overlap timing; supplemental verification took place between cohorts. The host is shared without affinity/isolation: unrelated processes remained running. No unrelated processes were stopped. Small effects should be read against the recorded process spreads.

The initial cohorts retain all earlier timings, including the table regression that prompted the known-run adjustment: [stage analysis and complete raw evidence](exploratory/readme.md). Do not combine their medians with the final cohort. [Final raw samples](final.jsonl) and [recomputed medians/spreads](final-summary.json) retain every process and batch. Process spread is `(maximum / minimum - 1) × 100` across the three process medians, not a confidence interval.

## Allocation and output preservation

[Allocation diagnostics](allocations.json) record one operation after caller-owned reusable storage has warmed. Parsing/setup is excluded except for complete HTML conversion. Output lengths/node counts agree in all 32 records. All four HTML conversions, five section-copy controls and three paired moves retain identical allocation/reallocation/requested-byte counts; moves allocate nothing.

The five short controls follow the same allocation pattern: plain code/math avoid one allocation, adjacent ASCII/Unicode request 186/155 fewer bytes, and punctuation is unchanged.

Ordinary Markdown keeps the same allocation counts. README avoids two reallocations and 39 requested bytes; the other three avoid one reallocation and 11 requested bytes each. Code/plain, code/marker-runs and math/plain each avoid one scratch allocation. Multiline code, math blocks, large code fences, frontmatter and tables retain their counts. Adjacent ASCII/Unicode text retains its allocation counts but requests 49,146/40,955 fewer cumulative bytes by avoiding speculative output growth. Punctuation-heavy adjacent text retains its counts and requested bytes. Requested bytes are cumulative allocation/growth requests, not peak or retained memory.

[Preservation results](preservation.json) record **8,468 unchanged corpus HTML/Markdown outputs**, **58,596 unchanged diagnostic results**, the original four through direct/checked/document HTML and Markdown serialization, and **120 unchanged complete edited-arena snapshots**. Diagnostic results compare HTML/Markdown values and errors, not complete serialized event sequences. Snapshots include detached nodes and syntax overrides, deserialize successfully in both versions, and retain their HTML. Serialization does not distinguish literal from borrowed/owned value storage.

All four source stages also agree on **21,098 new serializer outputs/diagnostics**: sixteen timed controls and 21,082 generated cases. These cover inline code/math strings of zero through three tokens drawn from ASCII, Unicode, markers, spaces/tabs, CR/LF, block openers, pipes and backslashes; every ASCII prefix plus Unicode/empty values; seven container contexts; text/soft-break values; both wrapping modes; long complete marker runs; malformed phrasing trees with invalid options; and table backticks/backslashes. Comparisons retain complete successful strings and complete diagnostic reason/source/rule/span fields. Separate verification asserts exact bytes before timing. Private regressions independently compare accepted lookahead prefixes with actual rendering across five escape scopes and both wrapping modes, and cover padding, physical line endings and sequential scratch reuse; the existing corpus also exercises nested formatting.

## Validation and reproduction

Native all-feature coverage passes **13,538/13,538 measured source regions**, with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Linux x86_64 all-feature library compilation passes as a **compile-only** local check; native runtime coverage is AArch64. [Validation](validation.json) records source identities, commands and log hashes. The complete evidence head is independently reviewed before opening the PR.

Use the [comparison setup](../../readme.md) and original fixture preparation. Generated inputs are ignored; [prepare.py](prepare.py) deterministically regenerates them, and metadata pins hashes. Historical fixture generators/harnesses are reused without modifying their source or saved evidence. Corpus attribution/licenses remain in those studies; new controls are generated structural inputs.

From the repository root, using source checkouts at the four pinned commits:

```sh
python3 benches/comparison/prepare.py
python3 benches/comparison/measurements/2026-10-25-serializer-rewrites-and-lookahead/prepare.py
python3 benches/comparison/measurements/2026-10-25-serializer-rewrites-and-lookahead/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate \
  --stage rewrite=/path/to/rewrite --stage scanner=/path/to/scanner --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-serializer-rewrites-and-lookahead/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-serializer-rewrites-and-lookahead/measure.py \
  --work /tmp/study --output /tmp/study/final.jsonl --mode html --mode markdown --mode controls \
  --input controls=benches/comparison/measurements/2026-10-25-serializer-rewrites-and-lookahead/all-controls.json
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/final.jsonl
```

Maintenance adapters enable `serde`; only the separate allocation executable enables its counting allocator. Original comparison adapters use their unchanged manifest/lockfiles. Frozen binaries live outside Cargo target directories. Use fresh output files for a new cohort: the driver resumes existing records and rejects changed input/binary identities. Verification reads archived probes and generated edit controls from earlier studies, prepared through the comparison setup and generator chain.
