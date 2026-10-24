# Block line-scan measurements

Production source and focused tests are in `01d053f8c97cc39520ef94046855626d5382fd84`; the baseline is merged main `ad900980b070239db52c90d145dc0d77f54d4d24`. The original four fixture hashes match the original benchmark. [Metadata](metadata.json) records revisions, binary/input/source hashes, machine, compiler, Ruby, and Inkmark versions. The [run manifest](run-manifest.json) records the final invocations, and the [harness guide](../../readme.md) describes engine options and timing boundaries.

## Retained changes

- `29308f6` skips the document root in container-prefix scans. The root is always open and cannot consume a source prefix.
- `01d053f` retains indentation already scanned while recognizing list and quote prefixes, instead of restoring the cursor and scanning it again. Blank lines still end at the same physical offset; pre-indentation positions and partially consumed tabs remain available to definitions, HTML, code, and table recognition.

There is no new cache, allocation, event representation, SIMD implementation, feature removal, or runtime dependency. The [exploratory record](exploratory/readme.md) includes rejected lookahead caching, leaf skipping, and first-byte interruption dispatch, plus every screening sample.

## Complete HTML rendering

Times are milliseconds per document: the median of three process medians, each containing seven calibrated samples after a 200 ms warmup. Baseline/candidate/Inkmark launch order alternates. Rust revisions use the same launch path, arguments, and environment. No project builds, tests, profiles, or reviewer workloads ran concurrently with timing runs; the machine was not otherwise isolated. All samples are retained.

| Workload | Merged main | Candidate | Inkmark 0.2.1 |
| --- | ---: | ---: | ---: |
| README 9 KB | 0.0147 | 0.0144 | 0.0181 |
| Mixed 100 KB | 0.4284 | 0.4202 | 0.3974 |
| Mixed 1 MB | 4.2897 | 4.2869 | 4.2631 |
| GFM 100 KB | 0.6422 | 0.6355 | 0.6946 |
| References | 0.6130 | 0.6129 | 0.8408 |
| Long prose 1 MB | 0.1209 | 0.1201 | 0.5579 |
| Fenced code 1 MB | 0.0736 | 0.0732 | 0.3034 |

Mixed-100k HTML takes 1.9% less time; mixed-1m has an essentially unchanged median (0.07% less). Each paired 1 MB run moves in the favorable direction by 0.07–1.35%, but this does not establish a large whole-document gain. README improves 2.4% and GFM 1.1%; references, long prose, and fenced code remain within 0.7% of baseline. See [summary](summary.json) for every process median. Differences this small remain sensitive to compiler/process layout and background variation.

The candidate takes 5.7% more time than Inkmark on mixed-100k and 0.6% more on mixed-1m; the latter is within ordinary variation. The overall-fastest objective is not met. CommonMark output agrees after entity decoding but is not byte-identical. **GFM differs in literal-link handling and table serialization**, so its timing is not an equal-output ranking. [Output agreement](benchmark-output-agreement.json) records these distinctions and confirms unchanged baseline HTML bytes.

Definitions at three sizes are single-process regression observations, not three-round rankings. The runtime versions are recorded in metadata; use the matched comparisons in this directory instead of attributing changes in historical absolute times to this patch.

## Preparation and event consumption

| Workload | Mode | Baseline ms | Candidate ms | Time reduction |
| --- | --- | ---: | ---: | ---: |
| mixed-100k | Preparation | 0.0956 | 0.0903 | 5.6% |
| mixed-1m | Preparation | 0.9660 | 0.9126 | 5.5% |
| mixed-100k | Public events | 0.3475 | 0.3396 | 2.3% |
| mixed-1m | Public events | 3.5399 | 3.4723 | 1.9% |
| mixed-100k | Arena parse and render | 0.9030 | 0.8815 | 2.4% |
| mixed-1m | Arena parse and render | 9.9796 | 10.0455 | -0.7% |

Preparation takes about 5.5% less time on the mixed workloads. Public event consumption improves about 2%. Arena results are less consistent: the candidate mixed-1m median is 0.7% slower. All arena samples, including the slow third candidate run and the slow first baseline GFM run, remain in [phase summary](phase-summary.json). No broad AST speedup is claimed.

Preparation includes parser construction and teardown. Public events include preparation, pulling, and event disposal without HTML. Arena mode includes parsing into the AST, traversal, HTML rendering, and disposal. These independently compiled modes must not be subtracted from HTML timings to infer renderer cost.

## Allocations and output consistency

Every recorded allocation/reallocation count and cumulative requested-byte field is unchanged, as are all public event and AST node counts. The AST node size remains 128 bytes. Source ranges and event boundaries are intentionally unchanged. All 2,117 saved HTML outputs are byte-identical across direct rendering, checked events, and arena rendering; see [preservation](preservation.json). Focused tests exercise indentation, blank lines, partial tabs, CR/LF/CRLF, nested quotes, definitions, and authored source offsets.

The separate allocation diagnostic’s first launch was terminated by SIGKILL after all timing runs had completed. The driver resumed without replacing or rerunning any timing samples, recreated the executable file at the same launch path, and completed all allocation observations. The termination cause was not established. The driver supports resuming already recorded invocations.

## Mixed-1m profile

A five-second macOS sample of unchanged main after a one-second warmup collected 4,160 samples. The [complete profile](main-mixed-1m-profile.txt), [flat view](main-mixed-1m-flat.txt), and [summary](main-profile-summary.json) are retained. Profiling ran separately from builds, tests, and timing benchmarks.

| Area | Exclusive samples |
| --- | ---: |
| Combined event-production/HTML loop | 32.6% |
| HTML escaping | 11.1% |
| Inline preparation | 9.9% |
| Block preparation’s own work | 7.4% |
| Memory copying | 6.0% |
| Event cleanup | 4.5% |
| Container scanning | 4.5% |

Block preparation including its helpers accounts for 21.8% of the samples; do not add that inclusive figure to the exclusive rows. The combined producer/renderer includes inlined parser work and cannot be labeled pure rendering cost. Much of the sampled copying occurs on HTML output paths, not allocator growth.

Our escape candidate scanner uses 16-byte ARM NEON chunks on this machine; replacement emission and short tails remain scalar. Inkmark enables pulldown-cmark’s SIMD feature, but its pinned pulldown-cmark-escape 0.11.0 gates its explicit SSSE3 HTML escaper to x86-64 and takes the scalar implementation on ARM. The exact published escape crate was checked against Inkmark’s lockfile checksum and matches the inspected upstream source; see [source verification](inkmark-escape-source.json), [Inkmark configuration](https://github.com/yaroslav/inkmark/blob/v0.2.1/ext/inkmark/Cargo.toml), and [escape source](https://github.com/pulldown-cmark/pulldown-cmark/blob/38e4d08f14ec4bd9783270e9623db7681ebed968/pulldown-cmark-escape/src/lib.rs). This is a source-level implementation observation, not an isolated escaper benchmark.

## Validation and reproduction

[Validation](validation.json) records 26,824/26,824 measured regions covered with 68 invariant-only exclusions, formatting, workspace Clippy, workspace tests, and an x86_64 Linux library build check. No architecture-specific code changed. [Independent review](independent-review.json) compared 305,217 source/option combinations: complete parser events, AST events, every source range and Text variant, and HTML were identical to baseline, with no panics or rendering-path disagreements. [Evidence review](independent-evidence-review.json) independently recomputed all 54 timing/phase summary groups and checked all 81 manifest input hashes and allocation records. The standalone review driver and deterministic case generator are archived alongside the results.

The archived Python drivers retain this session’s absolute paths and temporary baseline executables. Reproduce by building the recorded revisions with the same compiler, regenerating fixtures with prepare.py, and substituting paths while retaining a shared launch path and environment. The original four fixture contents and engine options are unchanged.
