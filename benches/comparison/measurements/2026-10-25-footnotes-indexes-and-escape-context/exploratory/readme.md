# Initial consolidation before fenced-body reservation

This study compares #58 with three consolidation changes: direct footnote backlink output, shared reference/footnote indexing helpers, and borrowed Markdown escape context. Public APIs, dependencies and successful output are unchanged. Inkmark was not retimed.

## Implementation

- Footnote backlinks use the shared HTML writer and escaping primitive. The renderer no longer assembles a separate backlink string or allocates escaped prefix/label strings and formatted suffix/superscript fragments. Each repeated backlink still allocates one number string, reused for its suffix and superscript; calls and footnote bodies still need their existing storage. Streaming produces finer-grained writer calls, so a writer failure can leave partial output within a backlink. The failure regression checks every write boundary, propagation and the last successfully written byte.
- Reference and footnote indexes share construction and binary-search helpers. Sorting breaks equal-key ties by authored position, retaining the first definition. Caller-owned vectors retain capacity, helpers are statically dispatched, and reference/footnote normalization remains distinct. The now-unused allocating HTML `encode` wrapper is removed from the private utility module.
- Escape rules read a fixed three-part view of borrowed preceding text, input and following text. This replaces the growing context string. A preflight check uses the same `Rule::at` predicate to skip rules that cannot affect the current input or immediately preceding escape. Possible rules retain the full prefix walk, preserving non-overlapping consumption. Collection retains only relevant escape positions, reducing sorting and retained scratch capacity. Input output is sliced directly from the original input.

The context helper adds code in exchange for removing an owned buffer and unnecessary historical work. It is not a claim that all serialization is linear: possible matches in escape-heavy adjacent nodes can still rescan the prefix. Trimming context to the last newline would be incorrect: consuming rules can carry their matching phase across chains such as `*\n*\n*\n`. The fixed view retains that behavior without caches, invalidation state or another escaping rule table.

## Source identities and results

Baseline **`c474387e6cd4d2c55360b3ee115ed40e7234e4ad`** is squash-merged #58. Intermediate sources are `0f133a8` (footnotes) and `4db2711` (shared indexes); final production source is **`4a97d2a437d188b7caa45505c58612f6a2b963f8`**, source tree **`ff89b8d11c2e86bb6270cb9156f25c412e612005`**. This initial source was independently approved before its timing; the parent study later adds fenced-body reservation. [Metadata](metadata.json) pins full commits/trees, frozen executable hashes, compiler versions, locks, driver/input hashes and document sizes. Evidence/docs are reviewed at the complete final head before the PR.

Original prepared-document Markdown takes **22.3–32.0% less time**. Original complete HTML ranges from **0.8% less to 1.1% more time**. Footnote-only feature controls take **15.6–35.6% less time**; reference-only conversion is approximately unchanged (-0.2%). Shared indexing is a consolidation, not an established independent speedup.

Dense adjacent ASCII/Unicode controls take **99.5%/99.6% less time** (198×/266× faster), and dense punctuation **95.2% less** (21× faster). Their short counterparts improve by **16.6%/33.0%/47.5%**. Large inline code/math controls improve by **69.9–86.7%**, while short code/math improve by **49.7%/43.5%**. These results justify retaining the small borrowed context abstraction, without claiming that prefix rescans are eliminated for every possible rule.

The 1 MB code-fence-form frontmatter control takes **67.5% more time** in the primary cohort (0.021389 → 0.035817 ms), with **73.3% candidate process spread**. Its successful output and allocation counts are identical. This regression is retained and investigated separately below; no arbitrary source padding or inlining was added to tune that control. Small math/table/frontmatter effects should also be read against their recorded spreads.

All timings are milliseconds; lower is faster. Time change is relative to baseline. Process spread is `(maximum / minimum - 1) × 100` across three process medians, not a confidence interval.

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.013127 | 0.013268 | +1.07% | 0.5% / 0.6% |
| html/mixed-100k | 0.392810 | 0.390785 | -0.52% | 0.8% / 2.2% |
| html/mixed-1m | 3.937781 | 3.905715 | -0.81% | 0.8% / 0.9% |
| html/gfm-100k | 0.609127 | 0.604565 | -0.75% | 0.7% / 1.5% |
| markdown/readme | 0.080466 | 0.062492 | -22.34% | 0.8% / 1.0% |
| markdown/mixed-100k | 2.011963 | 1.382937 | -31.26% | 1.7% / 1.9% |
| markdown/mixed-1m | 20.277036 | 13.781489 | -32.03% | 1.4% / 1.7% |
| markdown/gfm-100k | 2.217189 | 1.593287 | -28.14% | 0.8% / 1.4% |
| controls/code-plain | 215.120250 | 36.326617 | -83.11% | 0.5% / 0.7% |
| controls/code-marker-runs | 667.778042 | 200.858666 | -69.92% | 0.9% / 1.0% |
| controls/code-multiline | 69.987528 | 9.275125 | -86.75% | 0.8% / 0.9% |
| controls/math-plain | 175.720959 | 31.323100 | -82.17% | 3.2% / 1.1% |
| controls/math-flow-100k | 0.016654 | 0.016291 | -2.18% | 2.1% / 10.4% |
| controls/fenced-code-1m | 0.021389 | 0.035817 | +67.46% | 0.5% / 73.3% |
| controls/frontmatter-100k | 0.035838 | 0.035438 | -1.12% | 0.9% / 2.5% |
| controls/table-code-256 | 0.022812 | 0.022406 | -1.78% | 2.6% / 1.9% |
| controls/adjacent-ascii | 77.691146 | 0.392281 | -99.50% | 2.0% / 1.3% |
| controls/adjacent-unicode | 111.729083 | 0.419364 | -99.62% | 0.9% / 1.8% |
| controls/adjacent-punctuation | 33.154983 | 1.575992 | -95.25% | 1.7% / 0.9% |
| controls/code-plain-8 | 0.005301 | 0.002665 | -49.72% | 0.4% / 0.8% |
| controls/math-plain-8 | 0.004637 | 0.002620 | -43.50% | 1.1% / 2.0% |
| controls/adjacent-ascii-8 | 0.002084 | 0.001738 | -16.60% | 1.0% / 1.0% |
| controls/adjacent-unicode-8 | 0.002767 | 0.001853 | -33.03% | 0.8% / 0.6% |
| controls/adjacent-punctuation-8 | 0.013190 | 0.006924 | -47.51% | 1.1% / 0.7% |
| features/footnote-single | 0.001136 | 0.000960 | -15.56% | 1.4% / 1.6% |
| features/footnote-repeated-64 | 0.019993 | 0.012880 | -35.58% | 0.9% / 1.1% |
| features/footnotes-256 | 0.540923 | 0.432145 | -20.11% | 0.5% / 1.3% |
| features/footnote-blocks-and-options | 0.002503 | 0.001966 | -21.45% | 0.7% / 1.0% |
| features/references-4096 | 0.624497 | 0.623524 | -0.16% | 1.3% / 0.6% |
| features/duplicate-reference-and-footnote-keys | 0.222498 | 0.200120 | -10.06% | 1.2% / 0.4% |

Original HTML and feature controls include parsing, rendering and disposal. Original Markdown and serializer controls use prepared documents, including output disposal but excluding parsing. Original profiles and calibration come from the archived serializer/edit driver. Feature controls enable GFM and footnotes, raw HTML and all protocols; the options control includes escaped custom prefix/backlink-label values, Unicode identifiers, CRLF, multiple paragraphs and a code block. The 4,096-reference control and mixed duplicate-reference/footnote control exercise index construction and first-definition selection.

The sixteen serializer controls retain the previous study's generated inputs. The control named `fenced-code-1m` is recognized as **code-fence-form frontmatter**, because it appears at document start with a language tag and these controls enable frontmatter. The `frontmatter-100k` control uses the same form. The supplementary cohort adds `ordinary-code-fence-1m`, with preceding prose to ensure it exercises an ordinary code block. Names in historical studies are unchanged; their code-fence-form control also used frontmatter-enabled options.

Code/plain, code/marker-runs and math/plain use 2,048 literal/text pairs; code/multiline uses 1,024 pairs. The math block is a synthetic 100 KB leaf. Tables contain 256 code-containing rows. Large adjacent controls have 2,048 separate ASCII/Unicode text nodes or 512 punctuation triples. Short controls use eight repetitions. These edited-tree chunk boundaries need not survive ordinary parsing; they expose scaling behavior rather than representing typical document structure.

| Workload | Baseline ms | Footnotes ms | Indexes ms | Final ms |
| --- | ---: | ---: | ---: | ---: |
| controls/fenced-code-1m | 0.023665 | 0.035022 | 0.033298 | 0.034341 |
| controls/ordinary-code-fence-1m | 0.022624 | 0.026987 | 0.032293 | 0.022502 |

## Incremental stages

A separate cohort times six feature controls and five short serializer controls at all four source stages. Each stage is a complete operation, not an additive phase. These are separate process runs; do not combine their medians with the primary table.

| Workload | Baseline ms | Footnotes ms | Indexes ms | Final ms |
| --- | ---: | ---: | ---: | ---: |
| features/footnote-single | 0.001137 | 0.000955 | 0.000955 | 0.000960 |
| features/footnote-repeated-64 | 0.020035 | 0.012751 | 0.012886 | 0.012889 |
| features/footnotes-256 | 0.538941 | 0.426686 | 0.424084 | 0.428545 |
| features/footnote-blocks-and-options | 0.002458 | 0.001946 | 0.001948 | 0.001984 |
| features/references-4096 | 0.622476 | 0.627065 | 0.635034 | 0.624727 |
| features/duplicate-reference-and-footnote-keys | 0.222297 | 0.199671 | 0.199285 | 0.199342 |
| controls/code-plain-8 | 0.005289 | 0.005373 | 0.005289 | 0.002665 |
| controls/math-plain-8 | 0.004592 | 0.004658 | 0.004597 | 0.002587 |
| controls/adjacent-ascii-8 | 0.002086 | 0.002122 | 0.002092 | 0.001744 |
| controls/adjacent-unicode-8 | 0.002780 | 0.002741 | 0.002742 | 0.001838 |
| controls/adjacent-punctuation-8 | 0.013175 | 0.013097 | 0.013055 | 0.006903 |

[Primary samples](final.jsonl) and [summary](final-summary.json), [stage samples](stages.jsonl) and [summary](stages-summary.json), and [fence samples](fences.jsonl) and [summary](fences-summary.json) retain every completed batch/process. No samples were discarded or selected by outcome.

## Allocations and preservation

[Allocation records](allocations.json) count one warmed operation using a separate executable. Timing retains the native allocator. Counts exclude caller setup, except that complete HTML includes parsing. All 40 original/control/feature records (including two supplemental fence records) agree on output length or node count. Requested bytes are cumulative allocation/growth requests, not peak or retained memory.

All four ordinary Markdown workloads avoid one context allocation and seven reallocations. README requests 2,734 fewer bytes; the other three request 888 fewer bytes each. Adjacent ASCII/Unicode controls avoid one allocation and twelve reallocations, requesting 65,528 fewer bytes each. Punctuation-heavy adjacent text avoids one allocation and eighteen reallocations, requesting 49,016 fewer bytes. The 256-footnote control avoids 2,304 allocations and 1,536 reallocations; the 64-call control avoids 192 allocations and 65 reallocations. Reference-only, original direct HTML, code-fence-form frontmatter and AST edit allocation counts are unchanged. Full records include other inline and short controls.

[Preservation results](preservation.json) compare **8,468 corpus HTML/Markdown outputs**, **58,596 diagnostic results**, original samples through direct/checked/document HTML and Markdown, and **120 complete edited-arena snapshots**. Diagnostics compare returned values/errors, not complete event sequences. Snapshots retain detached nodes and syntax overrides and deserialize with identical HTML. Storage provenance is not distinguished between literal and owned text in serialized snapshots.

All four source stages agree on **23,100 serializer results** (sixteen primary controls, two supplemental fence records and 23,082 probes), plus six feature records containing complete HTML/Markdown with direct/document HTML parity. Probes retain the preceding 21,082 cases and add 2,000 adjacent-node context boundaries across paragraphs, headings, links, images and tasks. Private tests compare every UTF-8-valid three-way split of adversarial strings, rule matches and collector output; the existing independent regex comparison remains. They include CRLF, Unicode, indentation/digits and consuming-marker chains.

## Method, validation and reproduction

Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. Every cohort uses three sequential alternating rounds and fresh executable inodes, reversing engine order in round two. A document warms for at least 200 ms, then runs seven batches calibrated to approximately 150 ms (a single slow operation may exceed that). Tables are medians of three process medians. Input/options/document setup, process startup and JSON are outside timing. Own builds, tests, output/allocation diagnostics and source review did not overlap timing. The host is shared without affinity/isolation; unrelated processes were left running. Small effects and process spreads limit generalization.

Native all-feature coverage passes **13,781/13,781 measured regions**, with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Local Linux x86_64 all-feature library checking is compile-only; runtime Linux testing/coverage runs in CI. [Validation](validation.json) pins source identities and check-log hashes.

Use the [comparison setup](../../../readme.md) to prepare original fixtures. The [generator](../prepare.py) chains immutable historical generators and adds structural controls. Generated sources are ignored; metadata pins their hashes. Corpus attribution/licenses remain in the earlier studies. The new [adapter](../bench.rs) reuses calibration and the counting allocator from the serializer/edit study; only its allocation executable enables instrumentation. Build scripts enable `serde` for arena diagnostics without changing production manifests.

```sh
python3 benches/comparison/prepare.py
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/prepare.py
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate \
  --stage footnotes=/path/to/footnotes --stage indexes=/path/to/indexes --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/final.jsonl --mode html --mode markdown --mode controls --mode features
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/stages.jsonl --mode features --mode controls \
  --input controls=benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/stage-controls.json \
  --engine baseline --engine footnotes --engine indexes --engine candidate
python3 benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/measure.py \
  --work /tmp/study --output /tmp/study/fences.jsonl --mode controls \
  --input controls=benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/fence-controls.json \
  --engine baseline --engine footnotes --engine indexes --engine candidate
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/final.jsonl --input /tmp/study/stages.jsonl --input /tmp/study/fences.jsonl
```

Keep frozen binaries outside Cargo target directories and use a fresh output for a new cohort. The driver resumes completed records and rejects input/binary hash changes.
