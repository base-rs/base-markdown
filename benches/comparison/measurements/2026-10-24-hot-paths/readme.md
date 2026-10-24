# Parser hot-path experiments

Two retained optimizations, commits `64b27f9` and `bab3050`, are compared with merged main `4d18a7169bc9545f165c63e593d6a547e858c3c1` (PR #38). [Metadata](metadata.json) records exact source/executable hashes, toolchains, runtimes, machine, and input hashes. The original four fixture contents are unchanged. The baseline executable is byte-identical to PR #38's final candidate, whose source tree matches squash-merged main.

## Retained changes

- URI sanitization uses a static safe-byte lookup to scan ordinary runs, instead of matching several byte ranges at every byte. It omits empty writes around replacements, while preserving protocol checks, historical acceptance of two alphanumeric bytes after `%`, Unicode byte encoding, and writer-error propagation. The final source also hoists the common unchanged-prefix write before choosing an ampersand or percent replacement.
- NEON scanning retains the existing detection-only loop for chunks without matches. On a matching chunk, it selects matching lane indices and takes their minimum to return the first candidate, avoiding a scalar rescan. Short tails and the portable scalar path are unchanged. All vector loads remain within their slices.

There are no new allocations, runtime dependencies, public event changes, or removed features. Both changes are used through the existing semantic event contract and shared HTML renderer.

## Experiments and selection

Three agents worked in separate worktrees. Builds/tests used shared cached target directories one worktree at a time; each executable was copied before releasing its build slot. Root ran all timings sequentially with the same launch path, arguments, and environment. Builds, tests, profiles, and independent reviewer workloads did not overlap timing measurements. The machine was not otherwise isolated. [Candidate revisions and executable hashes](candidates.json), source patches, [initial screening](screening/initial/summary.json), [follow-up screening](screening/followup/summary.json), and [combined screening](screening/combined/summary.json) retain every sample. Screenings use two alternating rounds and are exploratory, not final rankings.

| Experiment | Observation | Decision |
| --- | --- | --- |
| Select the producer once per direct render. | Mixed rendering takes 3.9–5.2% more time; references 7.6% more. Source inspection suggests duplicated inlined rendering loops may hurt instruction footprint; this explanation was not verified in assembly. | Reject. |
| Extract SIMD lane positions on every chunk. | Long prose/code take 11–14% more time; mixed input does not improve. | Reject. |
| Extract lane positions only after the existing match detector succeeds. | Avoids the large safe-run regression; small possible mixed gain. | Retain for combined evaluation. |
| Classify safe URI runs using a byte lookup. | Repeated README/GFM/reference gains; mixed results initially small or flat. | Retain for combined evaluation. |
| Select the renderer destination once per sanitized URL. | Adds no improvement over the URI lookup alone. | Reject. |
| Expand compact grammar coverage. | Temporary instrumentation found no block or inline fallback in any original four fixture. HTML/multiline-code/path-link controls exercise fallback. | Defer; this cannot improve those fixtures through fallback avoidance. |

All five screened candidates preserve all 2,117 saved HTML outputs through direct, checked-event, and arena rendering; see [screening preservation](screening-preservation.json). The combined screening precedes a minor Clippy cleanup of the URI equality branch/common prefix write; the final measurements below use the exact updated source hashes in metadata. Candidate percentages must not be added across stages.

The [fallback probe](fallback-frequency.json) and its temporary [instrumentation patch](fallback-instrumentation.patch) measure producer selection and failed compact inline attempts, not execution time. Its `std` instrumentation is confined to a temporary diagnostic worktree and is not part of the production changes.

## Complete HTML rendering

Times are milliseconds per document: the median of five process medians, each containing seven calibrated samples after a 200 ms warmup. Baseline/candidate/Inkmark order alternates. All samples remain in the [manifest](run-manifest.json) and [summary](summary.json). No timing samples were discarded or replaced.

| Workload | Main | Candidate | Inkmark 0.2.1 | Candidate time reduction vs main |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 0.01418 | 0.01354 | 0.01857 | 4.5% |
| Mixed 100 KB | 0.41310 | 0.40711 | 0.40713 | 1.5% |
| Mixed 1 MB | 4.19929 | 4.13443 | 4.37971 | 1.5% |
| GFM 100 KB | 0.62321 | 0.60823 | 0.70335 | 2.4% |
| References | 0.59697 | 0.56651 | 0.85014 | 5.1% |
| Long prose 1 MB | 0.11599 | 0.11566 | 0.57556 | 0.3% |
| Fenced code 1 MB | 0.06988 | 0.06953 | 0.31464 | 0.5% |

The candidate and Inkmark are effectively tied at mixed-100k: a 0.005% difference is far below normal variation. The candidate takes 5.6% less time than Inkmark on mixed-1m in this run. CommonMark benchmark output agrees after entity decoding, not byte-for-byte. **GFM differs in literal links and table serialization**, so its engine timings are not an equal-output ranking; see [output agreement](benchmark-output-agreement.json).

All five paired mixed runs favor the candidate, but the gains are modest and remain sensitive to background activity and compiler/process layout. Long-prose/code differences are effectively flat. The three definition-size records are single-process regression observations, not repeated rankings; the largest candidate definition case takes 1.1% more time than main and remains slower than Inkmark. The overall-fastest objective is not established by this study.

## Checked rendering and arena construction

These complete-operation timings use three alternating baseline/candidate pairs. Checked rendering includes parsing, event validation, HTML rendering, and disposal. Arena mode includes parsing, arena construction, traversal, HTML rendering, and disposal. Do not subtract these independently compiled modes from direct rendering to infer phase costs.

| Workload | Mode | Main ms | Candidate ms | Time reduction |
| --- | --- | ---: | ---: | ---: |
| Mixed 100 KB | Checked events to HTML | 0.48262 | 0.46996 | 2.6% |
| Mixed 1 MB | Checked events to HTML | 4.84225 | 4.74036 | 2.1% |
| Mixed 100 KB | Arena parse and render | 1.12860 | 1.12248 | 0.5% |
| Mixed 1 MB | Arena parse and render | 12.20548 | 12.11093 | 0.8% |

Arena improvements are small; no large AST speedup is claimed. [Phase summary](phase-summary.json) includes all samples and README/GFM results.

## Profile and validation

A fresh five-second macOS sample of merged PR #38 after one-second warmup collected 4,144 samples. The combined event-production/HTML loop accounts for 34.3% of exclusive samples, inline preparation 9.5%, HTML escaping 8.5%, block preparation's own work 7.8%, memory copying 7.0%, event cleanup 4.8%, and URI sanitization 2.6%. The combined loop includes inlined parser work and is not pure rendering cost. These are selected exclusive categories, not a full partition; see the [raw profile](baseline-mixed-1m-profile.txt), [flat profile](baseline-mixed-1m-flat.txt), and [summary](baseline-profile-summary.json).

All [allocation records](allocation-agreement.json), including reallocation counts, cumulative requested bytes, event counts, AST node counts, and node size, match main. All [2,117 saved outputs](preservation.json) are byte-identical through direct, checked-event, and arena rendering. Tests cover every candidate byte across vector boundaries and alignments, multiple simultaneous matches, high bytes, and exact ASCII/percent URI encoding. Formatting, all-feature workspace Clippy, workspace tests, and local arm64 coverage pass: **26,922/26,922 measured regions**, with 68 invariant-only exclusions. The scalar-path library cross-compiles for x86_64 Linux. [Validation](validation.json) records the checks.

For reproduction, use the [comparison harness guide](../../readme.md), recorded toolchains/revisions, and pinned dependencies. The archived drivers retain session-specific executable and saved-corpus paths; substitute local paths while retaining a common Rust launch path/arguments/environment. `screen.py` records worktree screening, `diagnostics.py` checks candidate output and fallback frequency, `final-bench.py` records complete timings and supports resuming completed invocations, and `summarize.py` recomputes phase medians/input hashes/allocation agreement. The source patches reproduce rejected candidates without retaining their production changes.
