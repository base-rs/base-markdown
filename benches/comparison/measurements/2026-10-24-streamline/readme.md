# Streamlining parser state and HTML escaping

This study compares three implementation simplifications and a correction to scanner alignment with merged PR #39 (`50a4300a9aea81e8d72dcd189e128b995a7d8b36`). The original four benchmark inputs remain unchanged. Exact revisions, executable/source hashes, runtimes, input hashes, and every timing sample are retained alongside this report.

## Retained changes

- Compact blocks reuse the grammar producer’s existing inline workspace and dispatch state. The compact producer no longer retains two `InlineParser` instances. One workspace owns preparation, fallback selection, and delivery for each active region; regression coverage switches repeatedly between compact and grammar regions, including empty table cells and forward references.
- Paragraph tightness comes from its immediate item/list parents. Lists retain their own resolved tightness, while paragraphs no longer cache a copy. The list-close traversal that rewrote every direct child paragraph is removed. Nested lists and blockquotes retain their own container behavior.
- Direct events, captures, and legacy HTML helpers share one escape scanner and replacement writer. The separate scalar escaping loop is removed, so legacy helpers also use the existing bounded NEON scanner on arm64 and scalar fallback elsewhere. Raw HTML still replaces NUL, Unicode stays intact, and failed writes retain the last successfully written byte.

The line scanner also uses 64-byte alignment for stable stack placement after the parser-state changes. There are no new heap allocations, runtime dependencies, public API/event changes, or feature removals. This reduces duplicated ownership and mutation rather than replacing the event contract.

## Experiments

Agents prepared candidates in separate worktrees. Screening builds and tests were serialized through shared caches; final validation used separate target caches. All timings ran sequentially with the same executable launch path, arguments, and environment. Builds, tests, profiling, and reviewer work did not overlap timing windows. Other machine activity was not isolated.

[Candidate revisions, binary hashes, and code sizes](candidates.json), standalone patches, [initial screening](screening/initial/summary.json), and [follow-up screening](screening/followup/summary.json) retain every sample. The two-round screenings are exploratory; the longer combined comparison below determines the final result. Percentages from separate experiments must not be added.

| Experiment | Screening observation | Decision |
| --- | --- | --- |
| Share inline workspace. | Mixed time roughly flat to 1% lower; references 5.3% lower. | Retain; removes duplicate scratch state. |
| Derive paragraph tightness with an out-of-line helper. | Mixed time 5.6–5.8% higher; references 28.7% higher. | Revise. |
| Derive paragraph tightness with ordinary helper inlining and check node kind before reading parents. | Mixed time 0.4% higher; references effectively flat. | Retain for combined evaluation; removes the mutation traversal. |
| Relax all five forced-inline attributes. | Harness machine code section 3.9% smaller, but mixed time 7.4–7.6% higher. | Reject. |
| Relax only the renderer’s event-dispatch attribute. | Machine-code section 0.7% smaller, but mixed time 6.9–7.1% higher. | Reject. |
| Share the HTML encoder. | Original direct-render timings near flat; also removes the separate scalar loop used by legacy helpers. | Retain for combined evaluation. |

These measurements establish a runtime cost for the tested inlining changes; they do not prove that every forced-inline boundary is optimal. The initial paragraph helper was exported as a separate symbol, while its revised ordinary-inline implementation has local compiler copies; that observation alone does not attribute the entire timing difference to call overhead.

## Object storage

On `aarch64-apple-darwin`, `size_of::<EventParser>()` decreases from **1,416 to 1,176 bytes**, saving **240 bytes (17%)**. This is parser-object storage, including empty scratch-vector headers; it is not a 17% reduction in total memory or heap allocation. Block records and public event/arena node layouts are unchanged. The final multi-engine harness’s machine code section is 1,406,908 bytes versus 1,404,860 bytes for main (0.15% larger); source consolidation does not establish a binary-size reduction, and this executable also contains the other Rust engines. [Object-size measurements](parser-state.json) and the tiny [size probe](parser-size.rs) make the distinction explicit.

## Scanner layout correction

The first combined build exposed a repeatable constructor regression under the comparison’s gem environment: CommonMark preparation took about 1.43 ms on mixed 1 MB with `GEM_HOME` and `GEM_PATH` set, versus about 0.88 ms unset; main stayed near 0.92 ms. The parser does not read these variables. An unchanged executable reproduced the difference on both tested launch paths. The control without paragraph-tightness changes also reproduced it, so removing that simplification did not resolve the problem.

The [initial complete comparison](screening/pre-alignment/metadata.json), [phase confirmation](screening/phase-confirmation/summary.json), and [launch matrix](screening/launch-matrix/manifest.json) retain the slow observations. They were not discarded. Separate [profiles](screening/pre-alignment/profiles/prepare-profiles-metadata.json) localize the additional samples to stack-resident line-state loads/stores in block preparation. The exact hardware mechanism remains unproven; those short profiling windows are diagnostic observations, not a full phase partition.

Keeping container closure out of line did not resolve the sensitivity. Raising scanner alignment to 16 bytes reduced it, while 32 and 64 bytes resolved the tested gem-environment case. The final implementation uses 64-byte alignment and also passed [seven environment-padding checks](screening/aligned-layout-stress/metadata.json). These are diagnostic process observations rather than a repeated throughput ranking. Input content is identical to the original mixed-1m fixture; the diagnostic stores its JSON input compressed.

On the measured target, this pads each local `LineStart` from 48 to 64 bytes. Parser-object size and heap/arena layouts are unchanged. Alignment is safe and private; performance on other architectures and the precise CPU explanation are unverified. The final timings below use the corrected build. [All alignment and outlining candidates](candidates.json), patches, and launch matrices remain available.

## Complete HTML rendering

Times are milliseconds per document: the median of five process medians, each containing seven calibrated samples after a 200 ms warmup. Baseline/candidate/Inkmark order alternates. [Metadata](metadata.json), the [manifest](run-manifest.json), and [summary](summary.json) retain all invocations and samples. The Apple M4 Pro machine was not otherwise isolated.

| Workload | Main | Candidate | Inkmark 0.2.1 | Candidate time reduction vs main |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 0.01361 | 0.01368 | 0.01881 | -0.5% |
| Mixed 100 KB | 0.40761 | 0.40655 | 0.40875 | 0.3% |
| Mixed 1 MB | 4.18524 | 4.17001 | 4.37000 | 0.4% |
| GFM 100 KB | 0.61126 | 0.61216 | 0.71705 | -0.1% |
| References | 0.56845 | 0.55796 | 0.85523 | 1.8% |
| Long prose 1 MB | 0.11655 | 0.11546 | 0.58542 | 0.9% |
| Fenced code 1 MB | 0.06991 | 0.07001 | 0.32339 | -0.1% |

Original-four direct-render differences are below 1% and should be treated as effectively flat. References take 1.8% less time in these runs. Small differences remain sensitive to process layout and background activity; no overall throughput improvement is established for direct HTML.

Mixed-100k is effectively tied with Inkmark; mixed-1m takes about 4.17 ms versus 4.37 ms in this run. CommonMark benchmark output agrees with Inkmark after entity decoding, rather than byte-for-byte. **GFM differs in literal links and table serialization**, so its engine timings are not an equal-output ranking; see [output agreement](benchmark-output-agreement.json).

The three definition-size fixtures are single-process regression observations, not repeated rankings. The largest case takes 0.738 ms versus main’s 0.736 ms and Inkmark’s 0.683 ms. The overall-fastest objective is not established.

## Other complete operations

These complete operations use three alternating baseline/candidate pairs. Each includes preparation and disposal. Independently compiled modes must not be subtracted to infer additive phase costs.

| Workload | Operation | Main ms | Candidate ms | Time reduction |
| --- | --- | ---: | ---: | ---: |
| Mixed 100 KB | Prepare blocks and dispose | 0.09038 | 0.08704 | 3.7% |
| Mixed 1 MB | Prepare blocks and dispose | 0.91344 | 0.88756 | 2.8% |
| Mixed 100 KB | Prepare, consume all events, and dispose | 0.34435 | 0.31766 | 7.7% |
| Mixed 1 MB | Prepare, consume all events, and dispose | 3.48961 | 3.25440 | 6.7% |
| Mixed 100 KB | Checked events to HTML | 0.46895 | 0.47256 | -0.8% |
| Mixed 1 MB | Checked events to HTML | 4.76712 | 4.78838 | -0.4% |
| Mixed 100 KB | Arena parse and render | 0.89101 | 0.86100 | 3.4% |
| Mixed 1 MB | Arena parse and render | 9.59704 | 9.55367 | 0.5% |
| Long prose 1 MB | Legacy mdast parse and HTML render | 1.91297 | 0.95602 | 50.0% |
| Fenced code 1 MB | Legacy mdast parse and HTML render | 0.96250 | 0.34733 | 63.9% |

Event consumption takes about 7–8% less time on mixed inputs, while checked rendering is essentially flat and arena improvements are small. The legacy scanner fixtures improve substantially because legacy helpers now use the shared SIMD encoder. These legacy numbers include parsing and rendering; they are not scanner-only microbenchmarks. [Phase summary](phase-summary.json) includes all samples and README/GFM results.

## Validation and reproduction

All 2,117 saved outputs are byte-identical through direct HTML, checked-event rendering, and arena traversal; see [preservation](screening-preservation.json). All recorded allocation calls, reallocations, cumulative requested bytes, event counts, AST node counts, and node sizes match main; see [allocation agreement](allocation-agreement.json).

Formatting, all-feature workspace Clippy, workspace tests, and local arm64 coverage pass: **26,930/26,930 measured regions**, with 68 invariant-only exclusions. The scalar-path library cross-compiles for x86_64 Linux. Regression coverage exercises repeated compact/grammar transitions, nested list tightness, raw/NUL/Unicode escaping, and writer failures. [Validation](validation.json) records exact source revision and local logs. Other architectures’ runtime performance was not measured.

Use the [comparison harness guide](../../readme.md), pinned dependencies, and recorded toolchains/revisions. The archived drivers retain session-specific executable and saved-corpus paths; substitute local paths while retaining a common launch path, arguments, and environment. `screen.py` runs candidate screening, `preservation.py` checks the saved corpus, `final-bench.py` records the complete comparison and supports resuming recorded invocations, `output-agreement.py` checks engine outputs, and `summarize.py` recomputes phase medians and allocation agreement. `confirm-phases.py`, `launch-matrix.py`, and `layout-stress.py` retain the constructor diagnostics and layout checks. The compressed diagnostic input can be restored with `gzip -dk input.json.gz`.
