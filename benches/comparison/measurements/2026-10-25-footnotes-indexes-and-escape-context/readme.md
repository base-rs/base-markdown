# Footnote output, definition indexes, and borrowed escape context

This study compares #58 with three consolidation changes and fenced-body reservation: direct footnote backlink output, shared reference/footnote indexing helpers, and borrowed Markdown escape context. Public APIs, dependencies and successful output are unchanged. Inkmark was not retimed.

## Implementation

- Footnote backlinks use the shared HTML writer and escaping primitive. The renderer no longer assembles a separate backlink string or allocates escaped prefix/label strings and formatted suffix/superscript fragments. Each repeated backlink still allocates one number string, reused for its suffix and superscript; calls and footnote bodies still need their existing storage. Streaming produces finer-grained writer calls, so a writer failure can leave partial output within a backlink. The failure regression checks every write boundary, propagation and the last successfully written byte.
- Reference and footnote indexes share construction and binary-search helpers. Sorting breaks equal-key ties by authored position, retaining the first definition. Caller-owned vectors retain capacity, helpers are statically dispatched, and reference/footnote normalization remains distinct. The now-unused allocating HTML `encode` wrapper is removed from the private utility module.
- Escape rules read a fixed three-part view of borrowed preceding text, input and following text. This replaces the growing context string. A preflight check uses the same `Rule::at` predicate to skip rules that cannot affect the current input or immediately preceding escape. Possible rules retain the full prefix walk, preserving non-overlapping consumption. Collection retains only relevant escape positions, reducing sorting and retained scratch capacity. Input output is sliced directly from the original input.

The context helper adds code in exchange for removing an owned buffer and unnecessary historical work. It is not a claim that all serialization is linear: possible matches in escape-heavy adjacent nodes can still rescan the prefix. Trimming context to the last newline would be incorrect: consuming rules can carry their matching phase across chains such as `*\n*\n*\n`. The fixed view retains that behavior without caches, invalidation state or another escaping rule table.

Code-fence and frontmatter body writes reserve room for the body, closing fence and final line ending together, avoiding a second growth immediately after a large body. This uses the existing newline predicate and ordinary `String::reserve`; it adds no persistent state.

## Source identities and results

Baseline **`c474387e6cd4d2c55360b3ee115ed40e7234e4ad`** is squash-merged #58. Intermediate sources are `0f133a8` (footnotes) and `4db2711` (shared indexes); final production source is **`8126f8a5c4d1c43e64a49643b5063375e5aa8230`**, source tree **`d54347706f7d6ef3ae4edf816293abc8ecd774ea`**. The final source was independently approved before timing. [Metadata](metadata.json) pins full commits/trees, frozen executable hashes, compiler versions, locks, driver/input hashes and document sizes. Evidence/docs are reviewed at the complete final head before the PR.

Original prepared-document Markdown takes **24.6–32.3% less time**. Original complete HTML ranges from **0.4% less to 1.3% more time**. Footnote-only feature controls take **14.7–34.1% less time**. Reference-only conversion changes by **-0.4%**; shared indexing remains a consolidation, not an established independent speedup.

Dense adjacent ASCII/Unicode controls take **99.5%/99.6% less time**. These scaling gains, lower allocation counts and the ordinary-document improvements justify the borrowed context helper, while possible escape matches can still rescan the prefix. Every other measured control and its process spread is shown below; the results do not establish a universal ranking.

The initial source before reservation had a **67.5% slowdown** in the 1 MB code-fence-form frontmatter control, with 73.3% candidate process spread. Its separate repeat still measured a 45.1% slowdown; an ordinary code-fence repeat was approximately neutral but noisy. [All initial results](exploratory/readme.md) are retained. Reserving the body and closing markers together removes a measured reallocation; the final primary and separate fence cohorts below measure that source. We do not attribute all of the earlier timing variation to that allocation alone.

All timings are milliseconds; lower is faster. Time change is relative to baseline. Process spread is `(maximum / minimum - 1) × 100` across three process medians, not a confidence interval.

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| html/readme | 0.013217 | 0.013174 | -0.32% | 0.3% / 2.0% |
| html/mixed-100k | 0.391874 | 0.393054 | +0.30% | 1.8% / 0.6% |
| html/mixed-1m | 3.918375 | 3.967924 | +1.26% | 4.1% / 1.2% |
| html/gfm-100k | 0.607414 | 0.605012 | -0.40% | 27.8% / 0.9% |
| markdown/readme | 0.082654 | 0.062331 | -24.59% | 5.8% / 1.6% |
| markdown/mixed-100k | 2.025348 | 1.388643 | -31.44% | 2.6% / 2.0% |
| markdown/mixed-1m | 20.290609 | 13.736947 | -32.30% | 1.2% / 0.8% |
| markdown/gfm-100k | 2.227206 | 1.574347 | -29.31% | 1.7% / 0.6% |
| controls/code-plain | 216.248917 | 36.536283 | -83.10% | 25.2% / 2.6% |
| controls/code-marker-runs | 667.863875 | 201.742542 | -69.79% | 6.6% / 4.8% |
| controls/code-multiline | 69.934695 | 9.330120 | -86.66% | 48.2% / 4.5% |
| controls/math-plain | 175.514667 | 31.446742 | -82.08% | 8.7% / 4.7% |
| controls/math-flow-100k | 0.017002 | 0.015867 | -6.67% | 8.1% / 24.1% |
| controls/fenced-code-1m | 0.021468 | 0.021345 | -0.57% | 27.9% / 1.7% |
| controls/frontmatter-100k | 0.035751 | 0.035625 | -0.35% | 5.0% / 5.0% |
| controls/table-code-256 | 0.022815 | 0.022675 | -0.61% | 3.0% / 2.5% |
| controls/adjacent-ascii | 77.323583 | 0.385794 | -99.50% | 4.3% / 2.5% |
| controls/adjacent-unicode | 111.347230 | 0.413979 | -99.63% | 4.1% / 1.4% |
| controls/adjacent-punctuation | 32.613133 | 1.584107 | -95.14% | 3.7% / 2.3% |
| controls/code-plain-8 | 0.005308 | 0.002669 | -49.71% | 1.8% / 2.7% |
| controls/math-plain-8 | 0.004575 | 0.002584 | -43.52% | 2.9% / 3.1% |
| controls/adjacent-ascii-8 | 0.002072 | 0.001694 | -18.25% | 1.8% / 0.9% |
| controls/adjacent-unicode-8 | 0.002727 | 0.001843 | -32.44% | 3.3% / 2.0% |
| controls/adjacent-punctuation-8 | 0.013042 | 0.006913 | -46.99% | 1.9% / 2.0% |
| features/footnote-single | 0.001117 | 0.000952 | -14.73% | 1.4% / 2.6% |
| features/footnote-repeated-64 | 0.019843 | 0.013069 | -34.14% | 0.9% / 1.3% |
| features/footnotes-256 | 0.537088 | 0.433886 | -19.22% | 1.5% / 4.0% |
| features/footnote-blocks-and-options | 0.002438 | 0.001958 | -19.68% | 1.1% / 1.8% |
| features/references-4096 | 0.626227 | 0.623874 | -0.38% | 0.8% / 1.6% |
| features/duplicate-reference-and-footnote-keys | 0.221843 | 0.200226 | -9.74% | 1.4% / 1.4% |

Original HTML and feature controls include parsing, rendering and disposal. Original Markdown and serializer controls use prepared documents, including output disposal but excluding parsing. Original profiles and calibration come from the archived serializer/edit driver. Feature controls enable GFM and footnotes, raw HTML and all protocols; the options control includes escaped custom prefix/backlink-label values, Unicode identifiers, CRLF, multiple paragraphs and a code block. The 4,096-reference control and mixed duplicate-reference/footnote control exercise index construction and first-definition selection.

The sixteen serializer controls retain the previous study's generated inputs. The control named `fenced-code-1m` is recognized as **code-fence-form frontmatter**, because it appears at document start with a language tag and these controls enable frontmatter. The `frontmatter-100k` control uses the same form. The supplementary cohort adds `ordinary-code-fence-1m`, with preceding prose to ensure it exercises an ordinary code block. Names in historical studies are unchanged; their code-fence-form control also used frontmatter-enabled options.

Code/plain, code/marker-runs and math/plain use 2,048 literal/text pairs; code/multiline uses 1,024 pairs. The math block is a synthetic 100 KB leaf. Tables contain 256 code-containing rows. Large adjacent controls have 2,048 separate ASCII/Unicode text nodes or 512 punctuation triples. Short controls use eight repetitions. These edited-tree chunk boundaries need not survive ordinary parsing; they expose scaling behavior rather than representing typical document structure.

| Workload | Baseline ms | Candidate ms | Time change | Process spread B/C |
| --- | ---: | ---: | ---: | ---: |
| controls/fenced-code-1m | 0.032548 | 0.021376 | -34.32% | 22.6% / 1.1% |
| controls/ordinary-code-fence-1m | 0.030391 | 0.021653 | -28.75% | 19.9% / 0.8% |

The final primary code-fence-form frontmatter median is approximately unchanged (**0.6% less time**); its separate fence cohort measures **34.3% less time**, while the ordinary code fence takes **28.8% less time**. These cohorts must be read separately. Baseline fence timings remain variable (19.9–27.9% process spreads across the final fence measurements), while candidate spreads are 0.8–1.7%. The primary GFM HTML baseline has a 27.8% spread, and the primary multiline-inline-code baseline a 48.2% spread; all those samples are retained.

## Incremental stages

The initial cohort, retained under [exploratory evidence](exploratory/readme.md), times six feature controls and five short serializer controls at all four source stages before fence reservation. Its candidate is `4a97d2a`, not the final `8126f8a` below. Each stage is a complete operation, not an additive phase. These are separate process runs; do not combine their medians with the primary table.

| Workload | Baseline ms | Footnotes ms | Indexes ms | Before reservation ms |
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

[Primary samples](final.jsonl) and [summary](final-summary.json), [initial stage samples](exploratory/stages.jsonl) and [summary](exploratory/stages-summary.json), and [fence samples](fences.jsonl) and [summary](fences-summary.json) retain every completed batch/process. No samples were discarded or selected by outcome.

## Allocations and preservation

[Allocation records](allocations.json) count one warmed operation using a separate executable. Timing retains the native allocator. Counts exclude caller setup, except that complete HTML includes parsing. All 40 original/control/feature records (including two supplemental fence records) agree on output length or node count. Requested bytes are cumulative allocation/growth requests, not peak or retained memory.

All four ordinary Markdown workloads avoid one context allocation and seven reallocations. README requests 2,734 fewer bytes; the other three request 888 fewer bytes each. Adjacent ASCII/Unicode controls avoid one allocation and twelve reallocations, requesting 65,528 fewer bytes each. Punctuation-heavy adjacent text avoids one allocation and eighteen reallocations, requesting 49,016 fewer bytes. The 256-footnote control avoids 2,304 allocations and 1,536 reallocations; the 64-call control avoids 192 allocations and 65 reallocations. Reference-only, original direct HTML and AST edit allocation counts are unchanged. The 1 MB code-fence-form frontmatter control avoids one reallocation and 2,000,012 cumulative requested bytes; the ordinary code-fence control avoids one allocation, one reallocation and 2,000,031 requested bytes. The 100 KB frontmatter control avoids one reallocation and 200,044 requested bytes. Full records include other inline and short controls.

[Preservation results](preservation.json) compare **8,468 corpus HTML/Markdown outputs**, **58,596 diagnostic results**, original samples through direct/checked/document HTML and Markdown, and **120 complete edited-arena snapshots**. Diagnostics compare returned values/errors, not complete event sequences. Snapshots retain detached nodes and syntax overrides and deserialize with identical HTML. Storage provenance is not distinguished between literal and owned text in serialized snapshots.

All four source stages agree on **23,100 serializer results** (sixteen primary controls, two supplemental fence records and 23,082 probes), plus six feature records containing complete HTML/Markdown with direct/document HTML parity. Probes retain the preceding 21,082 cases and add 2,000 adjacent-node context boundaries across paragraphs, headings, links, images and tasks. Private tests compare every UTF-8-valid three-way split of adversarial strings, rule matches and collector output; the existing independent regex comparison remains. They include CRLF, Unicode, indentation/digits and consuming-marker chains.

## Method, validation and reproduction

Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. Every cohort uses three sequential alternating rounds and fresh executable inodes, reversing engine order in round two. A document warms for at least 200 ms, then runs seven batches calibrated to approximately 150 ms (a single slow operation may exceed that). Tables are medians of three process medians. Input/options/document setup, process startup and JSON are outside timing. Own builds, tests, output/allocation diagnostics and source review did not overlap timing. The host is shared without affinity/isolation; unrelated processes were left running. Small effects and process spreads limit generalization.

Native all-feature coverage passes **13,792/13,792 measured regions**, with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Local Linux x86_64 all-feature library checking is compile-only; runtime Linux testing/coverage runs in CI. [Validation](validation.json) pins source identities and check-log hashes.

Use the [comparison setup](../../readme.md) to prepare original fixtures. The [generator](prepare.py) chains immutable historical generators and adds structural controls. Generated sources are ignored; metadata pins their hashes. Corpus attribution/licenses remain in the earlier studies. The new [adapter](bench.rs) reuses calibration and the counting allocator from the serializer/edit study; only its allocation executable enables instrumentation. Build scripts enable `serde` for arena diagnostics without changing production manifests.

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
  --work /tmp/study --output /tmp/study/fences.jsonl --mode controls \
  --input controls=benches/comparison/measurements/2026-10-25-footnotes-indexes-and-escape-context/fence-controls.json
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/final.jsonl --input /tmp/study/fences.jsonl
```

Keep frozen binaries outside Cargo target directories and use a fresh output for a new cohort. The driver resumes completed records and rejects input/binary hash changes.

To reproduce the archived incremental stages, build with candidate `4a97d2a` in a separate work directory, then run `measure.py` with modes `features` and `controls`, input override `controls=.../stage-controls.json`, and engines `baseline`, `footnotes`, `indexes`, `candidate`. Its [separate metadata](exploratory/metadata.json) pins the complete cohort. Final primary/fence cohorts contain 30 process/mode records and 192 document/process results; initial cohorts retain another 60 records and 336 results.
