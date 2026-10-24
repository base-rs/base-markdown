# Source-backed text coalescing measurements

Production source and focused tests are in `7d853c742e2faf4f785246a483a90b5938755791`; the baseline is merged main `3f633aba6733e30566ec2b26c01bd683a7075031`. The original four fixture hashes match the original benchmark. [Metadata](metadata.json) records revisions, binary/input/source hashes, machine, compiler, Ruby, and Inkmark versions. The [run manifest](run-manifest.json) records every final invocation, and the [harness guide](../../readme.md) documents engine options and timing boundaries.

## Change and source provenance

The compact inline scanner now extends an escaped ASCII punctuation character through the following ordinary source run. For `\*stars`, it emits one source-backed `*stars` event instead of separate `*` and `stars` events. It reuses the existing SIMD/scalar candidate scanner and stops before syntax candidates; it does not merge unresolved delimiter/bracket nodes or concatenate disjoint source ranges into an owned string.

The same semantic events feed the HTML renderer and incremental AST builder, so fewer events also mean fewer AST text nodes. The public types and their metadata remain unchanged. Ordinary link destinations/titles and code content/language already have their own `Text::Source` ranges, independently of the event span. Coalesced escaped text retains the existing convention of excluding the escape backslash from its visible source slice. Decoded/constructed fields are not promised separate raw field ranges or a per-character source map. See the [architecture contract](../../../../context/pull-parser.md).

Text chunk boundaries and AST text-node counts intentionally change; rendered bytes do not. Tests verify source ranges, one-event AST budgets, editing a coalesced value without changing the backing source, all ASCII punctuation followed by Unicode, delimiter/code/entity/hard-break boundaries, and existing structured link/code detail ranges.

## HTML measurements

Times are milliseconds, medians of three process medians. Each process has 200 ms warmup and seven calibrated samples. Order alternates baseline/candidate/Inkmark and Inkmark/candidate/baseline. Both Rust revisions run from the same launch path with identical arguments and environment. Builds, tests, profiling, and reviewer CPU work finished before timing. Machine background activity was not isolated. Every sample is retained in `paired-*.jsonl`; [summary](summary.json) includes every process median.

| Workload | Merged main | Candidate | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0150 | 0.0147 | 0.0187 |
| Mixed 100 KB | 0.4418 | 0.4368 | 0.4088 |
| Mixed 1 MB | 4.4506 | 4.4128 | 4.3402 |
| GFM 100 KB | 0.6477 | 0.6488 | 0.7082 |
| References | 0.6167 | 0.6161 | 0.8493 |
| Long prose 1 MB | 0.1256 | 0.1227 | 0.5730 |
| Fenced code 1 MB | 0.0738 | 0.0737 | 0.3124 |

Mixed HTML takes 1.1% less time at 100 KB and 0.8% less at 1 MB; this is a modest result close to ordinary measurement variation. The initial two-pair screening measured 1.6% and 1.8% less time, respectively. GFM, references, and code remain within 0.2% of baseline in the final run. README and long-prose medians move by -1.5% and -2.3%, but they are not evidence of benefits from coalescing escaped text: their path has no relevant event reduction. Unchanged preparation also moves by up to 1.6%. Compiler/process layout and background variation remain material at this scale.

The remaining mixed gaps to Inkmark are 6.9% and 1.7%. The overall-fastest objective is not met. CommonMark benchmark output agrees with Inkmark after entity decoding, but is not byte-identical. **GFM still differs in literal-link handling and table serialization**, so its timing is not an equal-output ranking. [Output agreement](benchmark-output-agreement.json) records these distinctions and confirms all baseline HTML bytes are preserved.

Ruby is 4.0.9 in this run, versus 4.0.7 in the preceding HTML-write report; use this run's matched baseline/candidate/Inkmark comparison rather than attributing changes in historical absolute timings to this patch. Definitions at three sizes are single-process regression observations, not three-round rankings.

## Events and AST consumption

| Workload | Baseline events | Candidate events | Baseline AST nodes | Candidate AST nodes |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 385 | 385 | 311 | 311 |
| Mixed 100 KB | 17,030 | 16,514 | 12,643 | 12,127 |
| Mixed 1 MB | 169,424 | 164,290 | 125,784 | 120,650 |
| GFM 100 KB | 24,525 | 24,167 | 17,364 | 17,006 |
| References | 26,005 | 26,005 | 20,003 | 20,003 |

The mixed 1 MB case loses 5,134 events (3.0%) and the same number of AST nodes (4.1%). These are counted operations/nodes, independent of timing noise.

The following separate modes also use three alternating baseline/candidate process pairs, retained in `phase-*.jsonl` and [phase summary](phase-summary.json):

| Workload and mode | Baseline ms | Candidate ms | Time reduction |
| --- | ---: | ---: | ---: |
| Mixed 100 KB, public events | 0.3571 | 0.3464 | 3.0% |
| Mixed 1 MB, public events | 3.6086 | 3.5431 | 1.8% |
| Mixed 100 KB, AST parse and render | 0.9435 | 0.9165 | 2.9% |
| Mixed 1 MB, AST parse and render | 10.0486 | 9.8418 | 2.1% |

Public event consumption includes parser preparation, complete pulling, and event disposal without HTML. The AST mode includes parsing into the arena, event traversal, HTML rendering, and disposal; it is not an isolated AST-construction timer. Preparation is unchanged by this patch. These separately compiled modes must not be subtracted from direct HTML timings to infer renderer cost.

## Allocations

Every recorded allocation/reallocation count and cumulative requested-byte field is unchanged, including preparation, pulling, complete HTML, arena construction/traversal, and the legacy AST. Fewer live AST nodes do not imply a smaller allocated arena here: geometric vector capacities remain in the same growth buckets. The measured arena node size remains 128 bytes; arena traversal still allocates nothing.

Complete direct HTML allocation observations are:

| Workload | Allocation calls | Reallocations | Requested bytes |
| --- | ---: | ---: | ---: |
| README 9 KB | 17 | 13 | 58,313 |
| Mixed 100 KB | 13 | 21 | 1,187,168 |
| Mixed 1 MB | 13 | 27 | 10,712,376 |
| GFM 100 KB | 372 | 379 | 1,873,855 |
| References | 10 | 22 | 4,400,811 |

`allocations-*.jsonl` retain the full records and event/node counts. Input loading and options construction are excluded. Requested bytes are cumulative allocation/reallocation requests, not peak or retained memory. Allocation counting uses a separate executable so timing retains the system allocator. No strict heap-free parser claim is made.

## Validation and reproduction

All 2,117 saved outputs remain byte-identical across direct HTML, checked events, and arena traversal; see [preservation](preservation.json). [Independent review](independent-review.json) approved the exact source after 83,514 adversarial source/option combinations and 25 focused tests, with unchanged HTML and selected non-text metadata, no panics, and no discrepancies between rendering paths. Text chunk boundaries are deliberately excluded from byte-for-byte event comparisons.

[Validation](validation.json) records 26,833/26,833 measured regions covered with 68 invariant-only exclusions, formatting, Clippy, workspace tests, and an x86_64 Linux library build check. There is no new architecture-specific code, event representation, runtime dependency, or feature removal.

[Initial screening](exploratory/readme.md) retains every preliminary sample. No samples were discarded. The archived Python drivers retain this session's absolute paths and temporary baseline executables. Reproduce by building the recorded revisions separately with the same compiler, regenerating fixtures with `prepare.py`, and substituting paths while preserving the shared launch path and environment. The original four inputs and their engine options are unchanged.
