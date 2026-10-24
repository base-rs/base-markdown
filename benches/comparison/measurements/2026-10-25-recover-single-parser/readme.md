# Recovering unified parser throughput

The unified parser takes **3.5–7.0% less time than #49** on the original four examples. Those examples remain **1.2–2.3% slower than the pre-unification parser**, rather than the 4–9% regression originally measured in #49. The multiline 100 KB workload retains its approximately 4× throughput advantage over the former grammar fallback. These are paired local measurements, not a universal ranking.

## Implementation

The changes preserve one source-backed parser, the event/document/renderer APIs, all construct switches, and `no_std`. They add no parser fields, caches, alternate backend, dependency, or allocation. The source diff against #49 adds one net production line across five files; [the architecture](../../../../context/pull-parser.md) retains the same design.

- Skip literal-autolink label cleanup and mixed-strikethrough bookkeeping when those extensions cannot produce candidates.
- End paragraphs on blank continuations before block lookahead, and dispatch list, setext, and HTML scanners only for applicable opening bytes. Remove rejection checks whose private callers now establish the precondition.
- Keep the aligned list-lookahead cursor in its own helper so ordinary interruption dispatch does not need its stack frame.
- Scan definition destinations with a bounded source index and one cursor update. Destinations cannot cross a physical line; multiline labels and titles retain their existing fragment cursor and rollback rules.
- Keep unchanged raw inline values in a small source-backed helper, using the existing `memchr3` dependency to find NUL/CR/LF. Owned normalization remains in one shared routine with unchanged transformation rules.

A shorter rewrite of code/math event dispatch was discarded after [confirmation timings](exploratory/event-dispatch-confirmation.jsonl) regressed; its [patch](exploratory/event-dispatch.patch) and [metadata](exploratory/metadata.json) retain the experiment separately from the final measurements. The retained changes remove unnecessary work without introducing another representation or multiplying scanner specializations. Independent review checked correctness and the maintainability constraint at the final production commit.

## Direct HTML

Milliseconds per conversion; lower is faster. `baseline` in the raw data means merged #49 (`296a5b0`); `pre-unification` means #48 (`3d05ce1`); `candidate` is production commit `5d2c629`. [Metadata](metadata.json) records full source identities, `src` tree hashes, native executable hashes, host, compiler, and lockfiles.

| Workload | Before unification ms | #49 ms | Recovered ms | Inkmark ms | Time vs. #49 |
| --- | ---: | ---: | ---: | ---: | ---: |
| readme | 0.013244 | 0.014407 | 0.013402 | 0.018347 | -7.0% |
| mixed-100k | 0.383820 | 0.403629 | 0.389204 | 0.400714 | -3.6% |
| mixed-1m | 3.854409 | 4.060395 | 3.916267 | 4.291000 | -3.5% |
| gfm-100k | 0.591628 | 0.629352 | 0.604960 | 0.693754 | -3.9% |
| references | 0.534287 | 0.577711 | 0.566006 | 0.846716 | -2.0% |
| long-lines-1m | 0.107280 | 0.111245 | 0.108175 | 0.559545 | -2.8% |
| code-1m | 0.069227 | 0.070389 | 0.070241 | 0.305064 | -0.2% |
| definitions-256 | 0.039673 | 0.045867 | 0.043049 | 0.042179 | -6.1% |
| definitions-1024 | 0.163587 | 0.184423 | 0.172191 | 0.164820 | -6.6% |
| definitions-4096 | 0.713767 | 0.791289 | 0.744900 | 0.682477 | -5.9% |
| multiline-small | 0.004740 | 0.001560 | 0.001541 | 0.002497 | -1.2% |
| multiline-100k | 4.378086 | 1.083277 | 1.056094 | 1.205416 | -2.5% |

The four original medians are lower than Inkmark's in this run. **GFM has unequal output:** Inkmark leaves the workload's `www.` address unlinked and formats tables differently. The three original CommonMark samples match Inkmark after entity decoding, while the other CommonMark fixtures match byte for byte. All twelve baseline/candidate timed outputs are byte-identical. [Output agreement](output-agreement.json) records each comparison and output hash. Definition-heavy workloads still trail Inkmark and the pre-unification parser; the table retains those deficits.

The [raw timing records](html.jsonl) retain every calibrated sample; [the summary](html-summary.json) recomputes each sample median and then the median of three process medians. Each process warms for 200 ms and runs seven approximately 150 ms batches. Four engines run sequentially in alternating order. Startup and input loading are excluded; conversion and disposal, including normal Ruby GC, are included. This Apple M4 Pro Mac is shared, without CPU affinity. No local builds, tests, review, profiling, or allocation diagnostics overlapped final timing. Small differences require caution; no final samples were excluded.

[Phase diagnostics](phases.jsonl) separately measure prepare-and-drop and prepare-plus-event-consumption, including disposal. Preparation is broadly restored; prepare-plus-event-consumption still takes 4.8–7.8% more time than the pre-unification parser in this diagnostic run. These use one process per fixture and engine, rather than the three-round direct-HTML method. Their results must not be added together or treated as a formal attribution of each optimization's benefit.

## Compatibility and allocations

All **8,468 saved HTML/Markdown outputs** and **24,276 diagnostic results** remain byte-identical to #49. Diagnostics include the previous 23,124 cases and 1,152 additional destination/blank-line boundary cases: EOF and trailing escapes, empty angle destinations, LF/CR/CRLF titles, quote/list prefixes, partial-tab blank lines, and independently disabled indented code. They produce no panics or disagreement among direct HTML, checked events, and document traversal. [Preservation records](preservation.json) contain counts and hashes; the additional [boundary inputs](boundary-cases.jsonl.gz) supplement the inputs and probe driver in the [single-parser study](../2026-10-25-single-parser/readme.md).

All eight complete [allocation records](allocations.json) are identical to #49, including preparation, pulling, HTML, document construction/serialization, event counts, and node counts. Prepared AST traversal allocates nothing. Cumulative requested bytes are allocation traffic, not peak live memory. The multiline 100 KB path retains 1,313 allocations and 16 reallocations, compared with 58,435 and 7,810 before unification.

The native gate covers **13,344/13,344 measured source regions**, excluding 22 invariant-only unreachable regions. Default workspace Bake tests, default/all-feature all-target Clippy, formatting, and x86_64 Linux all-feature library cross-compilation pass. The Linux check is compile-only; native coverage exercises AArch64. [Validation metadata](validation.json) records the final production/review commit.

## Reproduction

Use the pinned [comparison setup](../../readme.md), build each source's adapter with the recorded compiler/locks, and keep immutable executables outside Cargo target directories. From this directory:

```sh
python3 measure.py --binary baseline=/tmp/study/main49 \
  --binary pre-unification=/tmp/study/main48 \
  --binary candidate=/tmp/study/recovered --binary inkmark=inkmark \
  --work /tmp/study --output /tmp/study/html.jsonl --rounds 3
python3 measure.py --binary baseline=/tmp/study/main49 \
  --binary pre-unification=/tmp/study/main48 \
  --binary candidate=/tmp/study/recovered --suite phases \
  --work /tmp/study --output /tmp/study/phases.jsonl --rounds 1
python3 summarize.py
```

`summarize.py` reads the recorded JSONL files beside it. To summarize a new run, copy those results here or adjust its study path. The baseline executable was frozen from #49's final production source before its squash merge; its `src` tree is identical to merged #49. The pre-unification executable is the immutable control used by that study, built with the same adapter and lockfiles.
