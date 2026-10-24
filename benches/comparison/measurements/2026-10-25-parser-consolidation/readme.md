# Parser ownership and event-delivery consolidation

Five separately committed simplification rounds remove the private `InlineWorkspace` type/module, give `EventParser` direct ownership of its existing data, share document-boundary handling, and consolidate block-child traversal. Production has **53 fewer net lines** (158 added, 211 removed), with unchanged parser size, allocations and semantic events. The original `InlineParser` implementation is byte-for-byte unchanged from #54 in the final source.

The final three-round comparison against #54 ranges from **0.61% less time to 0.67% more time on the original four workloads**. A separate repeat ranges from **0.81% less to 0.65% more**. Mixed-1m is **0.62% slower in the primary run and 0.65% slower in the repeat**. This is a small measured tradeoff, not a claim of exactly unchanged performance. Some definition/key cases have lower medians, but shared-host variability limits small-effect conclusions. There is no fresh Inkmark timing in this pass; [#54's comparison](../2026-10-25-inline-destinations/readme.md) remains separate.

## Implementation rounds

1. **Direct ownership:** move source/configuration references, definitions and reusable inline storage from `InlineWorkspace` to its sole consumer, `EventParser`. Remove the wrapper and accessors. The first screen exposed slower mixed/Clap medians, so the change was not treated as automatically neutral.
2. **Cold document completion:** move final-separator discovery and document completion from the always-inlined `next_event` body into the existing cold `document_boundary` handler. Start, body completion, terminal separator, document end and exhaustion retain the same state transitions. This adds no parser state or cache.
3. **Shared child traversal:** use one cloneable iterator over the prepared arena's sibling links for literal values, inline fragments, task eligibility, list-item endings, root tails and nested final-container tails. Each clone owns its current child index; yielded nodes borrow the immutable arena. No child collection is allocated.
4. **Specialization boundary:** select the existing reference/extension const combinations beside block preparation in `EventParser::prepare_inline`. Remove the intermediate wrapper introduced inside `InlineParser` in round one. Compiler symbols show the four combinations again, without adding inlining or layout annotations. This is a code-generation observation, not a separately isolated explanation for every timing difference.
5. **Carry nodes with indexes:** yield `(index, &Node)` together so consumers avoid repeated arena lookups. Inline-fragment mapping no longer captures a second arena pointer; empty/list-item ending cases use the iterator's optional result directly.

The SIMD scanners, parsing algorithms, event contract, document representation, public API and dependencies remain unchanged. Existing inlining attributes are unchanged. Logical source mapping, first-definition precedence, independently enabled extensions, footnote IDs, custom renderers and `no_std` remain supported. These are ownership and traversal changes, not a new parsing backend or input-streaming API.

## Final complete HTML conversion

Milliseconds per complete conversion; lower is faster. Baseline is squash-merged #54, **`b5b73356499260a94b06a5eae38c94d3d85652a9`**. Final production is **`561ae1d4d872c98874966742588516d8f4dd5a76`**, source tree **`8e564055055e70205d51bf2e3e6041ed159941c5`**. [Metadata](metadata.json) pins source, binaries, compilers, locks and all ten input/profile files.

| Workload | #54 ms | Final ms | Time vs. #54 |
| --- | ---: | ---: | ---: |
| readme | 0.013397 | 0.013452 | +0.41% |
| mixed-100k | 0.397552 | 0.400232 | +0.67% |
| mixed-1m | 3.930159 | 3.954484 | +0.62% |
| gfm-100k | 0.616869 | 0.613136 | -0.61% |
| references | 0.560080 | 0.557018 | -0.55% |
| long-lines-1m | 0.112761 | 0.112512 | -0.22% |
| code-1m | 0.072375 | 0.071670 | -0.98% |
| definitions-256 | 0.039478 | 0.038492 | -2.50% |
| definitions-1024 | 0.159252 | 0.152675 | -4.13% |
| definitions-4096 | 0.678029 | 0.677423 | -0.09% |
| multiline-small | 0.001606 | 0.001542 | -3.98% |
| multiline-100k | 1.050636 | 1.041067 | -0.91% |
| keys-uppercase-1024 | 0.138024 | 0.134205 | -2.77% |
| keys-unicode-1024 | 0.176569 | 0.176917 | +0.20% |
| keys-spaced-1024 | 0.157406 | 0.154124 | -2.09% |
| keys-random-1024 | 0.126085 | 0.121905 | -3.32% |
| keys-long-uppercase-1024 | 0.628100 | 0.617737 | -1.65% |
| keys-long-spaced-1024 | 0.651781 | 0.635200 | -2.54% |
| keys-unicode-dense-1024 | 0.408134 | 0.404700 | -0.84% |
| keys-unicode-long-1024 | 4.431482 | 4.299492 | -2.98% |
| keys-unicode-upper-long-1024 | 5.284661 | 5.238553 | -0.87% |
| keys-unicode-lower-long-1024 | 2.970941 | 2.981817 | +0.37% |
| keys-unicode-upper-tail-1024 | 4.836306 | 4.792174 | -0.91% |
| rust-readme | 0.009572 | 0.009515 | -0.60% |
| clap-readme | 0.005667 | 0.005609 | -1.01% |
| tokio-changelog | 1.086220 | 1.062428 | -2.19% |
| rust-nll-rfc | 0.183382 | 0.184497 | +0.61% |
| awesome-rust-readme | 1.017913 | 1.022140 | +0.42% |
| commonmark-spec | 0.484844 | 0.483392 | -0.30% |

[Raw primary timing](html.jsonl) retains all **60 process/profile records and 174 document/process results**. [The summary](html-summary.json) preserves every process median and spread. Some definitions, multiline and ASCII-key controls have 2–4% lower medians. Real-document changes range from 2.19% less time on Tokio to 0.61% more on the RFC. These measurements establish no general speedup, and the original mixed samples retain slightly slower medians.

## Separate paired repeat

The original four and all seven long/dense key controls were repeated in three sequential alternating rounds after the primary cohort. These **18 process/profile records and 66 document/process results** remain separate in [raw results](paired-repeat.jsonl) and [summary](paired-repeat-summary.json); they are never pooled with or substituted for primary samples.

| Workload | Primary time vs. #54 | Repeat time vs. #54 |
| --- | ---: | ---: |
| readme | +0.41% | +0.46% |
| mixed-100k | +0.67% | -0.81% |
| mixed-1m | +0.62% | +0.65% |
| gfm-100k | -0.61% | -0.70% |
| keys-long-uppercase-1024 | -1.65% | -1.16% |
| keys-long-spaced-1024 | -2.54% | -0.09% |
| keys-unicode-dense-1024 | -0.84% | -1.53% |
| keys-unicode-long-1024 | -2.98% | -0.04% |
| keys-unicode-upper-long-1024 | -0.87% | -3.16% |
| keys-unicode-lower-long-1024 | +0.37% | +0.43% |
| keys-unicode-upper-tail-1024 | -0.91% | +0.02% |

The large long-key slowdowns seen during the earlier fourth-round cohort do not recur as large median regressions with the final binary. They are still preserved in that earlier cohort. Shared-host drift and spikes prevent attributing the difference entirely to the fifth source change. Mixed-1m retains a small slower median in both final cohorts.

## Screening and intermediate results

Each screen uses two alternating rounds on the original four and six real documents. Round four also measures round three; round five also measures round four. These small cohorts guide implementation and remain distinct from the final three-round comparison. Their source commits, binary identities, raw results, summaries and per-round output checks are in [metadata](metadata.json) and [exploratory results](exploratory/).

| Screen | Source | Net production lines vs. #54 | Mixed-1m time vs. paired #54 | GFM time vs. paired #54 |
| --- | --- | ---: | ---: | ---: |
| round1 | `1e6c4f1` | -28 | +3.64% | +0.18% |
| round2 | `df2d20b` | -24 | +1.28% | -1.85% |
| round3 | `e380432` | -27 | +1.24% | +0.24% |
| round4 | `128604b` | -48 | +0.84% | -1.05% |
| round5 | `561ae1d` | -53 | +0.77% | -0.09% |

A full 29-input comparison was also completed after round four, before the node-reference refinement. It retains **2.05% more time on mixed-1m**, plus **13.96% and 10.61% more time on long uppercase/spaced ASCII keys**. The uppercase candidate process spread is **83.7%**, including an unusually slow 1.13 ms process median. [All fourth-round raw samples](exploratory/round4-full.jsonl), [summary](exploratory/round4-full-summary.json), [source/binary metadata](exploratory/round4-metadata.json) and [validation](exploratory/round4-validation.json) are retained. Final results belong to a different, explicitly identified source commit. No completed samples were deleted or selected out of any cohort.

[Compiler symbols](exploratory/scanner-symbols.json) record the preparation functions across all five sources and the baseline.

## Contract preservation and allocations

All **8,468 saved corpus HTML/Markdown outputs**, **24,276 existing probes** and **34,320 destination-boundary probes** match #54. Direct HTML, checked event rendering and document rendering agree on all 29 timed inputs. [Output preservation](preservation.json), [destination preservation](destination-preservation.json) and [agreement](output-agreement.json) retain input/output hashes.

A separate probe compares the **complete serialized semantic event sequence for all 58,596 diagnostic cases**, including event order, spans, tags, separator values and source/owned value representations. [Event preservation](event-preservation.json) retains cohort hashes and counts. It does not merely compare rendered HTML. Both `EventParser` layouts measure **520 bytes** on this compiler/target.

Every recorded allocation/reallocation count, cumulative requested-byte total, event/node count and 120-byte document node size is unchanged across all 29 inputs. [Allocator diagnostics](allocations.json) run in separate executables outside timing. Cumulative requested bytes are not peak memory. Borrowed child references do not introduce a retained collection.

Inkmark output checks still match 20/29 cases byte for byte and 28/29 after entity decoding. Generated GFM differs structurally (literal `www.` links and table formatting); all six real documents agree after entity decoding. This pass does not time Inkmark or update its ranking.

## Validation and reproduction

Final native coverage passes **13,521/13,521 measured source regions**, with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Linux x86_64 all-feature library compilation passes as a **compile-only** local check; runtime coverage here is AArch64. [Validation](validation.json) identifies the source and commands. Each additional source refinement was revalidated; final records refer to the fifth source. Independent source review approved it before final timing; the complete evidence head is independently reviewed before opening the PR.

Use the pinned [comparison setup](../../readme.md), generated fixtures and [immutable real-document manifest](../../real-documents.json). Native timing uses Homebrew Rust 1.99.1/LLVM 23.1.3 on an Apple M4 Pro; validation uses rustup stable Rust 1.99.1/LLVM 23.1.1. The host is shared without affinity or isolation, with an unrelated Fantail observer, Yugabyte and macOS background activity. Those processes were not stopped. All final timings and the repeat are outside this task's builds, tests, diagnostics and source review; every completed sample is retained.

Each process warms a document for at least 200 ms and measures seven calibrated approximately 150 ms batches. Reported values are the median of three process medians. Engines run sequentially in alternating order, with a fresh native launch inode per process. Startup, input loading and initial options construction are excluded; conversion, output disposal and the native allocator are included. The repeat uses the same frozen binaries as the primary run.

Build immutable baseline/candidate adapters from the recorded sources outside Cargo target directories. The baseline binaries are the frozen #54 adapters and their hashes match its study. [inputs.json](inputs.json) reuses all 29 established inputs. From the repository root, with binaries under `/tmp/study`:

```sh
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/measure.py \
  --binary baseline=/tmp/study/baseline --binary candidate=/tmp/study/candidate \
  --rounds 3 --inputs benches/comparison/measurements/2026-10-25-parser-consolidation/inputs.json \
  --work /tmp/study --output /tmp/study/html.jsonl
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/html.jsonl
python3 benches/comparison/measurements/2026-10-25-parser-consolidation/build-event-probes.py \
  --baseline /path/to/baseline-checkout --candidate /path/to/candidate-checkout --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-parser-consolidation/verify.py --work /tmp/study
```

Use fresh output paths; the measurement driver resumes existing records and rejects changed binary identities. The repeat omits `--inputs` and supplies `--input commonmark=benches/comparison/data/bench-commonmark.json --input gfm=benches/comparison/data/bench-gfm.json --input commonmark=benches/comparison/data/bench-key-controls.json`.

Verification expects matching `baseline-allocations`, `candidate-allocations`, `baseline-driver` and `candidate-driver` in addition to the two native adapters and event probes. Existing driver source/build instructions are in [the prior preservation study](../2026-10-25-normalize-identifiers/readme.md#validation-and-reproduction). [The event-probe builder](build-event-probes.py) uses the committed [probe source](event-probe.rs) and [lockfile](event-probe.lock); its separately pinned diagnostic dependencies do not change timing-adapter dependencies. `verify.py --events-only` skips already completed output/allocation checks. Input corpus/probe sources and their licenses/attribution remain in the linked prior studies and real-document manifest; this change adds no new Markdown corpus.
