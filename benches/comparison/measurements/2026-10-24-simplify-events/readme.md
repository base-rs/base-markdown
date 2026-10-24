# Simplifying event delivery and storage

This pass removes permanent AST bookkeeping, duplicate entity storage, and intermediate filtered-HTML strings. It compares merged main `49a38ba1dc4bbd38e22a939f7d7256323aefb610` with final production source `bdf9d9c0f4e74b6fbe2e77651fbdc7a3cff8c9a3`. The original four input hashes are unchanged. No public API, event semantics, features, or runtime dependencies change.

## Retained changes

- The AST builder keeps one last-child cursor for its active container. Opening a container resets the cursor; closing it makes that container its parent's latest child. Permanent nodes no longer need a last-child field. The existing open-container stack and incremental consumption budget remain unchanged.
- Compact document delivery uses explicit `Start`, `Body`, `DocumentEnd`, and `Done` phases instead of four interacting booleans. The final separator rules derive from the final leaf. A cold helper handles document boundaries outside the hot event loop, avoiding a second walk through the finished tree.
- GFM tag filtering emits source runs and escaped tag openers directly into the shared HTML writer, including heading and footnote captures. One scanner also serves the legacy String wrapper. `memchr` skips ordinary runs and ASCII-insensitive comparisons avoid allocating lowercase tag names.
- One bounded scanner recognizes character references for compact inlines and destination/title decoding. Inline candidates hold a static named-reference index or a numeric Unicode scalar, replacing a separate vector of decoded values. Named values need no temporary strings; numeric strings are constructed only when their events are pulled. Inline record size is unchanged.

These changes consolidate ownership and scanning within the existing pull contract. They introduce no event queue or second parsing backend. The [architecture](../../../../context/pull-parser.md) describes the resulting implementation.

## Complete HTML conversion

Times are milliseconds per conversion, lower is faster. Values are medians of three process medians, each with seven calibrated samples following at least 200 ms of warmup. Engine order alternates between rounds; both Rust revisions use the same executable launch path, arguments, and environment. Builds, tests, profiling, and independent review do not overlap timing. Normal machine background activity remains; no CPU affinity is set. The machine is an Apple M4 Pro running arm64 macOS, with Rust 1.99.1 and Ruby 4.0.9. [Metadata](metadata.json), [every raw sample](final-html.jsonl), and the [recomputed summary](final-html-summary.json) retain the exact inputs, invocations, and executable hashes.

| Workload | Main ms | Final ms | Inkmark 0.2.1 ms | Time reduction vs main |
| --- | ---: | ---: | ---: | ---: |
| README 9 KB | 0.01399 | 0.01386 | 0.01921 | 0.9% |
| Mixed 100 KB | 0.41361 | 0.40312 | 0.42041 | 2.5% |
| Mixed 1 MB | 4.31308 | 4.02921 | 4.47121 | 6.6% |
| GFM 100 KB | 0.62911 | 0.61153 | 0.72991 | 2.8% |
| References 80 KB | 0.56798 | 0.55842 | 0.86580 | 1.7% |
| Long prose 1 MB | 0.11735 | 0.11590 | 0.59139 | 1.2% |
| Fenced code 1 MB | 0.07091 | 0.07057 | 0.33434 | 0.5% |
| Entities 100 KB | 0.85965 | 0.83713 | 0.97902 | 2.6% |
| Ordinary HTML 100 KB | 0.37323 | 0.25644 | 0.33086 | 31.3% |
| Filtered HTML 100 KB | 0.39962 | 0.28496 | 0.30254 | 28.7% |
| Definitions 256 | 0.04178 | 0.04164 | 0.04451 | 0.3% |
| Definitions 1,024 | 0.17224 | 0.17059 | 0.17355 | 1.0% |
| Definitions 4,096 | 0.74800 | 0.75569 | 0.82888 | -1.0% |

The targeted HTML improvement is substantial and agrees with the isolated tag-filter experiment. Mixed results improve in this run, but the 6.6% mixed-1m result is larger than the 2.8% improvement in the earlier cold-boundary control. Treat the exact magnitude as run-dependent. Sub-percent differences are effectively flat; the largest definitions case takes about 1% more time than main.

Core-markdown has a lower median than Inkmark on these fixtures in this run. This establishes neither a universal fastest-parser ranking nor performance on other architectures. The mixed-100k lead is small. CommonMark README and mixed outputs agree after entity decoding, rather than byte-for-byte. **GFM is not an equal-output comparison:** core-markdown links `www.example.com`, while Inkmark leaves it literal; table serialization also differs. Linking that valid `www.` domain follows the [GFM autolink extension](https://github.github.com/gfm/#autolinks-extension-). Equivalent table formatting differences do not by themselves establish a correctness defect. This observation is not a claim of global conformance superiority. [Output agreement](benchmark-output-agreement.json) records every fixture's hashes and comparison.

The direct-render process medians also show background variation. Mixed-1m spans 4.160–4.618 ms for main and 3.987–4.095 ms for final; Inkmark spans 4.393–4.942 ms. Main GFM has one 1.393 ms process, compared with its other two at 0.616 and 0.629 ms. Inkmark mixed-100k has one 0.665 ms process, compared with 0.411 and 0.420 ms. No samples were removed.

## Other complete operations and layout checks

The [operation runs](phases.jsonl) use three alternating main/final pairs, with all sample medians checked in the [summary](phases-summary.json). Each mode includes construction and disposal; event draining adds consumption without rendering, checked HTML uses the public validated event path, and arena HTML builds and traverses the AST. These are separately compiled complete operations, not additive phases. Do not subtract them to estimate renderer cost.

| Workload / operation | Main median ms (process range) | Final median ms (process range) |
| --- | ---: | ---: |
| Mixed 1 MB, preparation | 0.89204 (0.885–0.895) | 0.88456 (0.879–0.894) |
| Mixed 1 MB, preparation + events | 3.50028 (3.323–5.888) | 3.16353 (3.161–3.176) |
| Mixed 1 MB, checked HTML | 4.90835 (4.883–10.828) | 4.80970 (4.768–4.814) |
| Mixed 1 MB, arena HTML | 9.97126 (9.939–25.859) | 9.28651 (9.185–9.527) |
| GFM 100 KB, preparation | 0.15020 (0.148–0.520) | 0.14902 (0.147–0.151) |
| GFM 100 KB, preparation + events | 0.52874 (0.488–1.253) | 0.47938 (0.477–0.479) |
| GFM 100 KB, checked HTML | 1.25702 (0.724–2.592) | 0.69501 (0.695–0.704) |
| GFM 100 KB, arena HTML | 1.81683 (1.340–1.977) | 1.28260 (1.282–1.288) |

Main's slow processes make some operation medians unstable, particularly GFM checked rendering and mixed-100k arena construction (1.301 ms median, spanning 0.886–1.498 ms). Final mixed-100k arena processes span 0.840–0.849 ms. The displayed mixed/GFM operation medians are lower, but do not establish the large percentage gains implied by the noisy medians. The object-size and allocation changes are the clearer evidence for reduced AST overhead.

README preparation alone takes 0.004004 ms versus main's 0.003641 ms, about 10% more time (0.36 μs). Its process ranges overlap: main spans 0.003555–0.003717 ms and final 0.003587–0.004161 ms. The complete README HTML operation remains effectively flat in the direct-render comparison. This diagnostic slowdown is retained; the measurements alone do not isolate its cause.

The [seven environment layouts](layout.json) vary only one inherited padding variable, alternating engine order while retaining the same launch path and original mixed-1m content. Preparation spans 0.881–0.901 ms for main and 0.877–0.911 ms for final, with both medians at 0.893 ms. The constructor slowdown addressed in the previous pass does not recur in these checks. These are one-process diagnostics per layout, not a repeated engine ranking; all samples are retained. `layout.py` reproduces the check.

## Object storage and allocation traffic

Measured arm64 object sizes are:

| Type | Main bytes | Final bytes |
| --- | ---: | ---: |
| `EventParser` | 1,176 | 1,152 |
| Arena `Node` | 128 | 120 |
| `AstBuilder` | 56 | 64 |
| Private `InlineParser` | 240 | 216 |
| Private inline record | 48 | 48 |

The parser loses one vector header, each arena node saves 8 bytes, and the builder gains one 8-byte cursor. These are object-storage measurements, not total or peak memory. [Compiler layouts](layouts.json) record the initial combined build; the [final public-size probe](final-parser-state.json) links the exact library used by the final benchmark and confirms its parser/node/builder sizes.

Complete-operation allocation measurements use a separate executable so timing retains the system allocator. Input loading and options construction are excluded. Allocation calls and reallocations are separate; requested bytes include cumulative growth requests and do not measure retained or peak memory.

| Workload / complete operation | Main allocation calls / reallocations | Final allocation calls / reallocations | Main requested bytes | Final requested bytes |
| --- | ---: | ---: | ---: | ---: |
| Mixed 1 MB, HTML | 13 / 27 | 12 / 27 | 10,712,376 | 10,712,280 |
| Mixed 1 MB, arena construction | 13 / 33 | 12 / 33 | 40,075,992 | 37,978,776 |
| Entities 100 KB, HTML | 6,908 / 2,310 | 4,607 / 2,309 | 563,042 | 560,454 |
| Ordinary HTML 100 KB, HTML | 9,439 / 9 | 4 / 9 | 1,682,405 | 1,571,072 |
| Filtered HTML 100 KB, HTML | 9,867 / 1,417 | 4 / 8 | 1,205,173 | 871,240 |

The [final raw allocations](final-allocations.json) are authoritative; event and node counts remain unchanged. Mixed-1m still has 120,650 arena nodes. The smaller node removes about 2.1 MB of cumulative arena allocation traffic in this workload. Arena traversal remains allocation-free. Numeric-reference allocation moves from inline preparation to delivery; compare complete operations instead of inferring savings by subtracting independently measured modes.

## Controls and rejected experiments

The [two-round screening](screen-summary.json) compares each independent simplification with main. Percentages across experiments must not be added. [Candidate revisions and hashes](candidates.json) and standalone patches retain every source variant, including experiments that were not kept.

The [initial combined comparison](html-summary.json) showed the original four HTML workloads about 1–3% slower, despite the targeted HTML gains. That observation is retained in full. A [control restoring the old completion state](phase-control-summary.json) did not consistently recover throughput: small cases improved slightly while mixed-1m and GFM worsened. Simply undoing the phase simplification did not resolve the regression.

The [outlining control](outline-control-summary.json) tested the original combination, cold document boundaries, an outlined filter helper, and both outlines in three alternating rounds. Keeping only document boundaries cold reduced mixed-1m time by 2.8% and GFM by 2.5% against main; small workloads stayed near flat. This is retained. Outlining tag filtering added complexity without a consistent benefit and was rejected. The final source also removes an invariantly true filter-tail condition found during coverage review; final timings use that exact source rather than the earlier control binary.

A [cached line-ending experiment](line-ending-control-summary.json) reused the style already computed during block preparation, removing a duplicate source scan. Its comparator is the cold-boundary candidate (`phase-final`, binary hash beginning `18f98957`), labeled `baseline` in this control's raw data; it is not merged main. Relative to that candidate, the experiment made mixed-1m about 2% slower while improving long prose about 1.7%; it was rejected. The final source retains the original style scan. This result and its patch remain available rather than being presented as an unmeasured simplification.

## Preservation, validation, and reproduction

All **2,117 saved outputs** remain byte-identical to main through direct HTML, checked-event rendering, and arena traversal: 652 CommonMark examples, 677 published GFM examples, 672 cmark-gfm examples, and 29 custom inputs in four profiles. [Preservation records](final-preservation.json) retain hashes for every corpus/path combination. This checks preservation of the baseline, not conformance of every possible input.

Exact final source passes formatting, all-feature/all-target workspace Clippy, default workspace tests including Bake and the generator, and native all-feature/all-target coverage: **27,101/27,101 measured source regions**, with 69 invariant-only unreachable exclusions. The library also cross-compiles for x86\_64 Linux; runtime performance and coverage on that platform were not measured locally. [Validation](validation.json) records source identity and log hashes. Regression cases exercise nested and empty AST containers, bounded consumption, document endings and repeated exhaustion, malformed/numeric/multiscalar references, fallback transitions, tag-name boundaries, captures, Unicode/NUL handling, and writer failures.

Use the [comparison harness guide](../../readme.md) and pinned lockfiles. Build the harness separately at the recorded baseline and final revisions with `cargo build --manifest-path benches/comparison/Cargo.toml --release --locked`, then preserve each executable outside the target cache. The evidence drivers accept those immutable executable paths. Regenerate the ordinary fixtures with the existing `prepare.py`; `fixtures.py WORK/data` generates the added deterministic entity/HTML fixtures without committing repeated 100 KB inputs. `measure.py` runs screening, complete HTML, operation modes, and controls; `summarize.py` recomputes every sample/process median. `verify.py`, `allocations.py`, and `output-agreement.py` collect preservation, allocation, and engine-agreement data. Some archived drivers retain session-specific corpus and executable paths; substitute local paths while keeping the common launch path and matched environment.

For example, from the repository root after preparing the original fixtures and copying the two executables:

```sh
python3 benches/comparison/measurements/2026-10-24-simplify-events/fixtures.py /tmp/markdown-study/data
python3 benches/comparison/measurements/2026-10-24-simplify-events/measure.py \
  --binary baseline=/tmp/markdown-study/baseline \
  --binary final=/tmp/markdown-study/final --binary inkmark=inkmark \
  --rounds 3 --suite html --work /tmp/markdown-study \
  --output benches/comparison/measurements/2026-10-24-simplify-events/reproduction.jsonl
```

`measure.py` supports resuming without discarding recorded samples and rejects changed executable hashes. Timings include parsing, rendering where applicable, result disposal, and ordinary Ruby GC; process startup, input I/O, and initial options construction are excluded. All raw samples, including slow and rejected variants, remain in this directory.
