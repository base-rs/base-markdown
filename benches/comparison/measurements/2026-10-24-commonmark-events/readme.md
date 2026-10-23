# Event delivery and buffer measurements

The final production source is `8d384208abbdf77acba6f445496d16214169d04b`; merged baseline is `dc4f5ee536ce4b718f87c862debca52daa3f44b3`. The original four input hashes match the original benchmark. See [metadata](metadata.json) for the machine, compiler, engine versions, binary hashes, and input hashes, and [run manifest](run-manifest.json) for every command. The [harness guide](../../readme.md) describes the engine options and measurement boundaries.

## Retained changes

- Replace fixed literal-link prefix strings with private tags. On the recorded compiler, inline `Kind` shrinks from 24 to 16 bytes and each reusable inline `Node` from 56 to 48 bytes. These are compiler observations, not ABI guarantees or sizes of the public AST/event types; see [layout evidence](layout.json).
- Inline semantic endpoint lookup and compute a block closing endpoint once.
- Initially reserve the smaller of the input byte length and 4 KiB for direct HTML. Output grows normally beyond that. Invisible-only documents can retain the initial capacity, and reservation precedes extension parse errors; callers can supply their own writer through the event API.

## HTML measurements

Times are milliseconds, medians of three process medians. Each process uses 200 ms warmup and seven calibrated samples. Order alternates baseline/candidate/Inkmark and Inkmark/candidate/baseline, with identical launch path, arguments, and environment for Rust revisions. Tests, builds, and reviewer CPU work finished first. Machine background activity was not isolated, and small differences remain sensitive to compiler/process layout. Every raw sample is retained in the `paired-*.jsonl` files; [summary.json](summary.json) includes process medians. Original workloads, references, and scanner regressions all receive three rounds.

| Workload | Merged main | Candidate | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0155 | 0.0149 | 0.0186 |
| Mixed 100 KB | 0.4401 | 0.4371 | 0.4048 |
| Mixed 1 MB | 4.4920 | 4.4437 | 4.2797 |
| GFM 100 KB | 0.6578 | 0.6512 | 0.6998 |
| References | 0.6465 | 0.6488 | 0.8430 |
| Long prose 1 MB | 0.1224 | 0.1219 | 0.5673 |
| Fenced code 1 MB | 0.0736 | 0.0736 | 0.3128 |

HTML improvements are modest: README takes 4.0% less time, mixed 100 KB 0.7%, and mixed 1 MB 1.1%. References change by +0.4%, long prose by -0.5%, and code by +0.1%; these small changes are effectively neutral. Mixed 100 KB and 1 MB remain 8.0% and 3.8% slower than Inkmark. The overall-fastest target is not met.

The three definition sizes are single-process regression observations, retained in `definitions-*.jsonl` and the summary. They are not three-round ranking claims.

## Public events and allocations

Preparing and draining the public event iterator falls from 3.785 to 3.528 ms on mixed 1 MB (6.8% less time), and from 0.559 to 0.511 ms on GFM (8.4% less time). These modes include preparation and event disposal but no rendering; they are separate compiled loops and must not be subtracted from HTML timings to infer renderer cost.

[Phase summary](phase-summary.json) and `phase-*.jsonl` retain three alternating baseline/candidate rounds of preparation and complete event consumption. Neither mode materializes an AST or HTML.

Complete HTML allocation counts are:

| Workload | Baseline calls / reallocations | Candidate calls / reallocations | Baseline requested bytes | Candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 17 / 22 | 17 / 13 | 71,041 | 58,313 |
| Mixed 100 KB | 13 / 30 | 13 / 21 | 1,191,736 | 1,187,168 |
| Mixed 1 MB | 13 / 36 | 13 / 27 | 10,716,944 | 10,712,376 |
| GFM 100 KB | 372 / 388 | 372 / 379 | 1,878,423 | 1,873,855 |
| References | 10 / 31 | 10 / 22 | 4,929,155 | 4,400,811 |

Requested bytes are cumulative allocation/reallocation requests, not peak or retained memory. Input loading and options construction are excluded. Counts use a separate executable so timing retains the system allocator. `allocations-*.jsonl` also separates preparation, pulling, AST construction, and traversal. Arena traversal remains allocation-free. No strict heap-free parser claim is made.

## Correctness and rejected experiments

[Preservation evidence](preservation.json) records all 2,117 saved outputs across direct HTML, checked events, and arena rendering. [Benchmark output agreement](benchmark-output-agreement.json) confirms unchanged baseline HTML. The original CommonMark workloads agree with Inkmark after entity decoding, but are not byte-identical. GFM still differs in literal-link handling and table serialization, so its timing comparison does not establish an equal-output ranking.

[Independent review](independent-review.json) approved the exact source after 28,569 focused source/option combinations, with complete driver output identical to baseline and no panics or rendering/selected-metadata discrepancies. [Validation](validation.json) records 26,909/26,909 measured regions covered, 68 invariant-only exclusions, workspace tests, lint, and the portable build check.

[Exploratory measurements](exploratory/readme.md) retain all diagnostic samples, including rejected producer specialization, dispatch outlining, scanner changes, and capacity policies. The [superseded full-source reservation](uncapped-output/readme.md) includes its complete evaluation and three-round scanner isolation: its roughly 9% long-prose regression led to the bounded reservation. None of the rejected abstractions, new SIMD code, or feature removals are included.

The archived Python drivers retain this session's absolute paths and temporary baseline executables; they record the exact local invocation rather than provide a checkout-independent command. For reproduction, build the recorded revisions separately with the same compiler, regenerate fixtures with `prepare.py`, and substitute those paths while keeping the launch path and environment identical between revisions. No samples were discarded when selecting final reported runs; superseded sources are clearly separated.
