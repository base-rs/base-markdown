# One source-backed Markdown parser

The source-backed block and inline parser now handles every supported construct configuration. The grammar tokenizer, token-edit pipeline, and compatibility dispatch are deleted: **18,251 net production lines removed**. The semantic event contract, incremental AST builder, custom event renderers, source-backed document, and `no_std` support remain. The [architecture](../../../../context/pull-parser.md) describes the implementation.

Ordinary fragments retain their original source ranges. Only syntax crossing discontinuous fragments requests a reusable logical view and source map, then runs the same scanner. Definition lookahead is lazy, contiguous metadata returns source ranges directly, and labels, titles, and string decoding reuse the shared candidate-byte scanner, including its existing AArch64 NEON implementation. No alternative Markdown engine or new runtime dependency is introduced.

## Source and method

Baseline is merged main `3d05ce1f0da355161447f5b0a1f1f710c9855fde`. Final production source is at `17c5f2d7c2861784afe06c54cb338c4aa8b14962`; subsequent report-only commits leave it unchanged. [Metadata](metadata.json) records source trees, executable hashes, compiler, host, lockfiles, and background CPU activity.

Measurements ran locally on Apple M4 Pro, arm64 macOS, Homebrew Rust 1.99.1, Ruby 4.0.9, and Inkmark 0.2.1. Each HTML result is the median of three process medians, each with seven calibrated samples after 200 ms warmup. Engine order alternates; native adapters run at the same path using fresh inodes. Startup and input loading are excluded; conversion, disposal, and normal Ruby GC are included. Builds, tests, profiling, allocation diagnostics, and source review did not overlap final timing. This shared machine has background processes and no CPU affinity; small differences need caution.

## Direct HTML

The original four conversions take **4–9% more time than main**. The multiline 100 KB workload takes **75.5% less time** (4.07× the throughput), and its small counterpart is 2.95× faster. These are measured tradeoffs for one implementation, not an across-the-board speedup. Inkmark remains faster on definition-heavy fixtures; all four original medians favor the unified parser over Inkmark in this run, with the mixed-100 KB margin small enough to need caution.

Milliseconds per conversion, lower is faster. [Raw samples](html.jsonl), [recomputed summary](html-summary.json).

| Workload | Main ms | Unified ms | Inkmark ms | Time vs. main |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013418 | 0.014589 | 0.018753 | +8.7% |
| mixed-100k | 0.388571 | 0.404339 | 0.409330 | +4.1% |
| mixed-1m | 3.890872 | 4.048453 | 4.421857 | +4.1% |
| gfm-100k | 0.592230 | 0.630369 | 0.705450 | +6.4% |
| references | 0.544068 | 0.583457 | 0.846540 | +7.2% |
| long-lines-1m | 0.109006 | 0.112069 | 0.582177 | +2.8% |
| code-1m | 0.069726 | 0.071454 | 0.323232 | +2.5% |
| definitions-256 | 0.040131 | 0.046466 | 0.042611 | +15.8% |
| definitions-1024 | 0.166102 | 0.188378 | 0.165024 | +13.4% |
| definitions-4096 | 0.729171 | 0.812140 | 0.684438 | +11.4% |
| multiline-small | 0.004802 | 0.001625 | 0.002453 | -66.2% |
| multiline-100k | 4.469455 | 1.096946 | 1.207766 | -75.5% |

The GFM comparison has unequal output: Inkmark leaves the fixture's `www.` address unlinked and formats tables differently. See [output and compatibility](#output-and-compatibility) for the remaining output comparisons.

[Preparation/event diagnostics](phases.jsonl) measure prepare-and-drop and prepare-plus-event-consumption separately, including disposal. [Their summary](phases-summary.json) retains the ordinary-input overhead visible in direct HTML. The [invalid-URL scaling run](scaling.jsonl) takes 0.099, 0.384, and 1.703 ms for 1,024, 4,096, and 16,384 repeated invalid prefixes. This checks that failed domains reuse terminal lookahead rather than repeatedly scanning the same suffix; it is one diagnostic run, not a complexity proof for every Markdown input.

## Output and compatibility

All **8,468 saved outputs** remain byte-identical to baseline: 2,117 corpus/profile combinations through direct HTML, checked semantic events, arena HTML, and Markdown serialization. The **23,124 diagnostic probes** comprise 10,944 extracted/targeted cases, 12,000 generated syntax/options cases, and 180 label-boundary cases. They produce no panics or disagreement among the three HTML consumers. This is regression evidence, not an exhaustive conformance or fuzzing proof. [Preservation records](preservation.json), compressed [extracted probes](cases.jsonl.gz) and [generated probes](generated-cases.jsonl.gz), and the [probe driver](probe-driver.rs) preserve the evidence.

Diagnostic probes expose intentional corrections outside the saved output corpus: 523 complete-result differences, including 153 HTML differences. Multiline quoted reference labels no longer include stripped container prefixes in lookup keys; tables accept headers and rows previously rejected by the grammar dispatch; delimiter behavior follows the shared CommonMark rules; Markdown escaping no longer depends on the old grammar's text chunks. The compressed [extracted-case differences](differences-cases.json.gz) and [generated-case differences](differences-generated-cases.json.gz) retain old/new results. Focused public regressions cover corrected syntax and independently disabled constructs, including unresolved footnote-label fallback, expanded tab limits, and empty/BOM-only frontmatter.

Every timed baseline/candidate fixture has byte-identical HTML. [Agreement records](benchmark-output-agreement.json) also compare Inkmark. **GFM timings have unequal output:** Inkmark leaves the fixture's `www.` address unlinked and formats tables differently. The original three CommonMark samples agree after entity decoding rather than byte for byte; the other CommonMark fixtures agree byte for byte. Timings therefore do not establish a universal parser ranking.

## Allocations and representation

The original four fixtures retain identical preparation, inline-pull, direct-HTML, document-construction, and traversal allocation traffic, as well as event/node counts. Traversing a prepared document still allocates nothing. [Allocation records](allocations.json) measure cumulative requested bytes, not peak live memory.

The multiline 100 KB conversion changes from **58,435 to 1,313 allocations** (97.8% fewer), **7,810 to 16 reallocations**, and **27,468,611 to 3,772,859 cumulative requested bytes** (86.3% less traffic). Its 40,241 events and 28,558 AST nodes remain unchanged. Reusing inline scratch and removing grammar token machinery make this improvement much larger than the ordinary-input changes.

The arm64 [layout probe](layout.json) measures `EventParser` growing from 368 to **520 bytes**: logical-view, mapping, code-info, and footnote vector headers now live in the single parser. Their buffers allocate only when needed; the boxed grammar workspace is gone. `DocumentNode` remains 120 bytes and `AstBuilder` 64 bytes. This stack-size tradeoff is explicit rather than hidden behind a backend enum.

## Validation

The native all-feature/all-target gate covers **13,325/13,325 measured source regions**, excluding 22 invariant-only unreachable regions. Default workspace Bake tests, default and all-feature all-target Clippy, formatting, and Linux x86_64 all-feature library cross-compilation pass. Linux cross-compilation is compile-only; the native gate exercises the NEON implementation. [Validation metadata](validation.json) records configurations. Independent adversarial review approved the final source and its follow-up tests.

The `log` Cargo feature remains accepted as a no-op after removing tokenizer tracing. Public parser/event/document APIs retain their signatures; no syntax feature was removed in this consolidation.

## Reproduction

Use the pinned [comparison setup](../../readme.md), build baseline/final adapters separately, and keep immutable executables outside Cargo target directories. Then run:

```sh
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary candidate=/tmp/study/final --binary inkmark=inkmark \
  --work /tmp/study --output /tmp/study/html.jsonl --rounds 3
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary candidate=/tmp/study/final --suite phases \
  --work /tmp/study --output /tmp/study/phases.jsonl --rounds 1
python3 measure.py --binary candidate=/tmp/study/final --suite scaling \
  --work /tmp/study --output /tmp/study/scaling.jsonl --rounds 1
python3 summarize.py
```

`summarize.py` reads this study's recorded JSONL files. The phase and invalid-URL scaling runs are diagnostics with one process per case; they are not three-round ranking results. Earlier frozen implementations remain in [initial measurements](exploratory/initial-html.jsonl) and [batched-scanner measurements](exploratory/batched-html.jsonl), each with its own [initial metadata](exploratory/initial-metadata.json) or [batched-scanner metadata](exploratory/batched-metadata.json).
