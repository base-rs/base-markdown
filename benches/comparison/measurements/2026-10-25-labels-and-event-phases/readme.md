# Reference labels and event phases

This change simplifies two internal loops while retaining the single source-backed parser. Paired HTML medians take **0.8–1.3% less time on README and the mixed examples**, and **0.3% more on GFM**, compared with merged #50. The largest definition workload improves by **1.4%**; the smaller definition workloads are essentially unchanged. These are small local differences, with mixed results on the controls, rather than a broad throughput breakthrough.

## Implementation

Reference normalization copies whole non-whitespace runs located by the existing SIMD-capable byte scanner, then skips Markdown whitespace. This replaces a byte-at-a-time loop and its separate start/index/whitespace state. Trimming, collapsed spaces, ASCII uppercasing, and whole-string Unicode lower-then-upper folding remain unchanged. No new storage, dependency, or parser specialization is introduced.

Inline event delivery is a parser phase instead of an independent boolean. Pending separators still precede further block/inline delivery; empty inline regions close normally, and document completion remains terminal. This removes a redundant state field. Isolated screening against normalization alone was near neutral: HTML differences ranged from approximately −0.2% to +0.7%, and prepare-plus-event-consumption from +0.1% to +1.0%. **No independent speedup is claimed for the phase change.** [Exploratory records](exploratory/metadata.json) distinguish those runs from the final comparison.

The comparison guide now separates recorded results, reproduction, output caveats, current architecture, and historical studies. Historical reports and raw data are preserved.

Five-second sampling profiles of #50 identified normalization and key comparisons as substantial costs on definitions-4096. The mixed-1m event-consumption profile concentrated in the inlined event-production closure, inline preparation, event disposal, and block parsing. Sampling cannot precisely attribute individual inlined instructions. This change reuses the existing scanner and consolidates delivery state; it does not introduce another key representation or replace sorting with a new index/cache. [Profile records](profiles/metadata.json) retain the sampled executable identity and compressed reports.

## Direct HTML

Milliseconds per conversion; lower is faster. `baseline` is merged #50 (`2e98603`), and `candidate` is production commit `f237aca`. [Metadata](metadata.json) records full source identities, executable hashes, host, compiler, and lockfiles.

| Workload | #50 ms | Candidate ms | Inkmark ms | Time vs. #50 |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013766 | 0.013592 | 0.018943 | -1.3% |
| mixed-100k | 0.401132 | 0.397763 | 0.409984 | -0.8% |
| mixed-1m | 3.995614 | 3.950691 | 4.349333 | -1.1% |
| gfm-100k | 0.608085 | 0.609922 | 0.704936 | +0.3% |
| references | 0.569801 | 0.566545 | 0.854892 | -0.6% |
| long-lines-1m | 0.110944 | 0.112012 | 0.578419 | +1.0% |
| code-1m | 0.071176 | 0.072470 | 0.321865 | +1.8% |
| definitions-256 | 0.043574 | 0.043626 | 0.042608 | +0.1% |
| definitions-1024 | 0.175677 | 0.175295 | 0.167450 | -0.2% |
| definitions-4096 | 0.759563 | 0.748802 | 0.692826 | -1.4% |
| multiline-small | 0.001570 | 0.001580 | 0.002538 | +0.7% |
| multiline-100k | 1.043837 | 1.056738 | 1.217184 | +1.2% |

Core-markdown has lower medians than Inkmark on nine of twelve workloads in this run. Definition-heavy workloads still take approximately **2–8% more time**. The long-line, code, and multiline controls retain small regressions against #50; this report does not hide those results or establish that every workload improved.

**GFM has unequal output:** Inkmark leaves the workload's `www.` address unlinked and formats tables differently. The three original CommonMark examples match Inkmark after entity decoding; the other timed CommonMark examples match byte for byte. All twelve baseline/candidate outputs are byte-identical. [Output agreement](output-agreement.json) records comparisons and output hashes.

[Raw timing records](html.jsonl) retain every calibrated sample. [The summary](html-summary.json) recomputes each sample median and then the median of three process medians. Engines run sequentially in alternating order. Each process warms for 200 ms and runs seven approximately 150 ms batches. Startup and input loading are excluded; conversion and disposal, including normal Ruby garbage collection, are included. This Apple M4 Pro Mac is shared, without CPU affinity. No local builds, tests, independent review, profiling, or allocation diagnostics overlap final timings. No final samples are excluded. Differences near 1% are sensitive to noise and compiler/process layout.

## Event diagnostics

Separate [phase records](phases.jsonl) measure complete prepare-and-drop and prepare-plus-event-consumption operations, including disposal. They use three alternating process rounds against #50, without HTML rendering. Preparation ranges from −1.7% to +0.2% time versus #50. Prepare-plus-event-consumption ranges from −0.2% to +1.9%; it does not show a broad improvement, and the mixed 1 MB diagnostic is slower in all three rounds. These are independent operations, not additive phases or a formal attribution of the changes' individual benefits. [The phase summary](phases-summary.json) retains all process medians.

## Compatibility and allocations

All **8,468 saved corpus HTML/Markdown outputs** and **24,276 diagnostic results** remain byte-identical to #50, with no panics or disagreement among direct HTML, checked events, and document traversal. [Preservation records](preservation.json) identify inputs and output hashes; the inputs and diagnostic driver are retained by the [single-parser](../2026-10-25-single-parser/readme.md) and [recovery](../2026-10-25-recover-single-parser/readme.md) studies.

All eight previously recorded complete allocation/event/node records are unchanged. Four additional reference/definition workloads are recorded without a historical allocation control. [Allocation records](allocations.json) cover preparation, pulling, HTML, document construction/traversal/serialization, and event/node counts. Prepared document traversal allocates nothing. Cumulative requested bytes are allocation traffic, not peak live memory. The parser remains `no_std`; its public API and semantic event representation are unchanged.

Native coverage passes **13,363/13,363 measured source regions**, excluding 22 invariant-only unreachable regions. Default workspace Bake tests, default/all-feature all-target Clippy, formatting, and x86_64 Linux all-feature library compilation pass. The Linux check is compile-only; local runtime coverage exercises AArch64. Focused regressions cover non-Markdown Unicode whitespace, Unicode folding expansion, scanner chunk boundaries, empty inline regions, one-event consumption budgets, separators, and repeated terminal pulls. [Validation metadata](validation.json) records the reviewed source.

## Reproduction

Use the pinned [comparison setup](../../readme.md), recorded compiler/locks, and immutable executables outside Cargo target directories. The baseline executable was frozen from #50's final production source (`5d2c629`), whose `src` tree equals squash-merged #50. From this directory:

```sh
python3 measure.py --binary baseline=/tmp/study/main50 \
  --binary candidate=/tmp/study/candidate --binary inkmark=inkmark \
  --work /tmp/study --output /tmp/study/html.jsonl --rounds 3
python3 measure.py --binary baseline=/tmp/study/main50 \
  --binary candidate=/tmp/study/candidate --suite phases \
  --work /tmp/study --output /tmp/study/phases.jsonl --rounds 3
python3 summarize.py
```

`summarize.py` reads the recorded JSONL files beside it. Copy new results there, or adjust its study path, to summarize a new run. The multiline input is reused from the recovery study and identified by its input hash in each record.
