# Fixed HTML write measurements

The retained production change is `aa2c98ca57b0aa5f79e3631e1b4695c54d3d2a9c`; focused regression tests are in `01a1a079ad62c2ffa21891dbaf068275af1de5bf`. The baseline is merged main `3750a5289e1998d1eeb4c762f137a376d5af7f28`. The original four input hashes match the original benchmark. [Metadata](metadata.json) records source revisions, binary and input hashes, machine, compiler, and engine versions; the [run manifest](run-manifest.json) records every final command. The [harness guide](../../readme.md) documents options and measurement boundaries.

## Retained change

Emit fixed heading/list/table/code/task markup and adjacent attribute delimiters as complete static strings. Dynamic values use the same escaping and URI sanitization; all writes retain the same capture routing and error propagation. No temporary strings, allocations, new event representation, or public API are introduced. Public event validation and private structural checks remain intact.

On the 1,000,020-byte mixed document, the instrumented checked renderer makes 218,195 logical `fmt::Write` calls instead of 243,865: 10.5% fewer. Event count, text bytes, 5,134 empty writes, and 1,472,348 output bytes are unchanged. These are logical writer calls, not OS writes or a count of calls surviving compiler optimization for `String`. See [baseline counts](writer-shape-baseline.json), [candidate counts](writer-shape.json), and [instrumentation](writer-shape.rs).

## HTML measurements

Times are milliseconds, medians of three process medians. Each process uses 200 ms warmup and seven calibrated samples. Order alternates baseline/candidate/Inkmark and Inkmark/candidate/baseline. Both Rust revisions run from the same launch path with identical arguments and environment. Builds, tests, profiling, and independent review CPU work finished before timing; machine background activity was not isolated. Every sample is retained in `paired-*.jsonl`; [summary](summary.json) includes all process medians.

| Workload | Merged main | Candidate | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0150 | 0.0149 | 0.0184 |
| Mixed 100 KB | 0.4402 | 0.4313 | 0.3987 |
| Mixed 1 MB | 4.4696 | 4.4054 | 4.2833 |
| GFM 100 KB | 0.6444 | 0.6420 | 0.6954 |
| References | 0.6478 | 0.6143 | 0.8418 |
| Long prose 1 MB | 0.1215 | 0.1220 | 0.5676 |
| Fenced code 1 MB | 0.0734 | 0.0734 | 0.3075 |

The candidate takes 2.0% less time on mixed 100 KB, 1.4% less on mixed 1 MB, and 5.2% less on references. README changes by -1.0%, GFM by -0.4%, long prose by +0.4%, and fenced code by less than 0.1%; these small changes are effectively neutral. The earlier exploratory long-prose +2.3% observation did not repeat at that size in this final run. Mixed improvements were smaller in the two-pair exploratory run, so do not treat a 1–2% difference as a universal result. References improved in both evaluations.

Mixed 100 KB and 1 MB remain 8.2% and 2.9% slower than Inkmark. The overall-fastest objective is not met. CommonMark benchmark HTML agrees after entity decoding but is not byte-identical. **GFM differs in literal-link handling and table serialization**, so its timing does not establish an equal-output ranking. See [output agreement](benchmark-output-agreement.json).

Definitions are single-process regression observations at three sizes, retained in `definitions-*.jsonl` and the summary; they are not three-round ranking claims.

## Public phases and allocations

The parser is unchanged. Preparation and complete public event consumption remain effectively neutral in the three alternating pairs recorded in [phase summary](phase-summary.json) and `phase-*.jsonl`. For example, mixed 1 MB event consumption changes from 3.543 to 3.563 ms (+0.6%), and preparation from 0.941 to 0.945 ms (+0.4%). These are separate compiled loops and must not be subtracted from HTML timings to infer renderer cost.

All recorded allocation measurements are unchanged, including complete HTML:

| Workload | Allocation calls | Reallocations | Requested bytes |
| --- | ---: | ---: | ---: |
| README 9 KB | 17 | 13 | 58,313 |
| Mixed 100 KB | 13 | 21 | 1,187,168 |
| Mixed 1 MB | 13 | 27 | 10,712,376 |
| GFM 100 KB | 372 | 379 | 1,873,855 |
| References | 10 | 22 | 4,400,811 |

Counts exclude input loading and options construction. Requested bytes are cumulative allocation/reallocation requests, not peak or retained memory. Allocation runs use a separate executable so timing retains the system allocator. `allocations-*.jsonl` also records preparation, pulling, arena construction/traversal, and legacy AST construction. This is not a heap-free parser claim.

## Correctness and rejected experiments

All 2,117 saved outputs remain byte-identical across direct HTML, checked events, and arena traversal; see [preservation](preservation.json). All benchmark outputs are also byte-identical to baseline. [Independent review](independent-review.json) approved the exact source and tests after 28,569 differential combinations, 50 synthetic cases, and 188 injected writer failures. Focused tests check exact checkbox serialization in both checkability modes and inject failures at every observed write position, including ordered lists starting at one.

[Validation](validation.json) records 26,827/26,827 measured regions covered with 68 invariant-only exclusions, formatting, Clippy, workspace tests, and an x86_64 Linux library build check. No architecture-specific implementation changed.

[Exploratory evidence](exploratory/readme.md) retains every two-pair screening sample and the rejected patches. Skipping more private-path validation, skipping empty escape fragments, and choosing the output route once per escaped value did not improve mixed throughput enough to retain. The final source keeps only fixed-markup batching. No quote serialization, SIMD implementation, or feature set changed.

The archived Python drivers retain local absolute paths and temporary baseline executables; they record exact invocations rather than provide checkout-independent commands. To reproduce, build the recorded revisions separately with the same compiler, regenerate fixtures using `prepare.py`, and substitute binary paths while keeping launch paths and environment identical. No samples were discarded; screening experiments and final retained-source runs are kept separately.
