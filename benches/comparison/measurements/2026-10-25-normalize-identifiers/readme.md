# Unicode identifier folding and real-document comparison

A small shared helper skips the intermediate lowercase string when a normalized label contains no non-ASCII uppercase character. In this local comparison with merged #52, the original Unicode-label control takes **10.6% less time**, and long lowercase Unicode labels take **33.3% less time**. Both avoid **1,027 allocations per conversion**. The late-uppercase stress case takes **7.5% more time** because it scans the whole label before retaining both string conversions.

The candidate has lower medians than Inkmark on all twelve established workloads and all six new real documents. Across all twenty-nine inputs it has twenty-eight lower medians; the mixed dense-Unicode control remains **3.7% slower than Inkmark**. These are local measurements, with differing output on the generated GFM workload and substantial shared-host variability. They do not establish a universal ranking or a broad improvement on ordinary documents.

## Implementation and tradeoff

The ASCII whitespace scanner, borrowing check, sorted definition index, source-backed event/document representation, and public APIs are unchanged. Both reference and footnote normalization call one Unicode folding helper. It uses Rust's complete string lower-then-upper conversion if any non-ASCII uppercase character occurs, preserving contextual casing and expansions. Otherwise direct string uppercasing produces the same result, including ASCII uppercase, titlecase and uncased characters. Reference keys still lowercase their ASCII letters afterward; visible footnote IDs retain uppercase.

The helper replaces two identical expressions with one shared decision; it adds no fields, reusable buffers, caches, dependencies or generated tables. It still allocates an uppercase result and retains both case-conversion strings when needed. It is not allocation-free or an in-place Unicode conversion.

Tests compare the previous string-based result for every Unicode scalar, plus contextual sigma, combining marks, expansions, non-Markdown whitespace and scratch reuse. The [Rust casing documentation](https://doc.rust-lang.org/std/primitive.char.html#method.to_lowercase) explains why character-wise lowercasing alone differs from string lowercasing around Greek sigma; the retained implementation uses the full string operation whenever uppercase Unicode is present.

The predicate is intentionally conservative. A late uppercase scalar can make the scan additional work with no allocation saving. The upper-tail control retains that cost explicitly rather than hiding it among lowercase examples.

## Complete HTML conversion

Milliseconds per conversion; lower is faster. `baseline` is squash-merged #52 (`d0d0a82`); the candidate is **`7663402ac2a9f0414e99f71f98f0d226b35b0504`**, source tree **`d980bef94cd839e92d6fe623238e66c671eca7bf`**. [Metadata](metadata.json) records source/binary identities, compiler, locks and every input hash. The three original CommonMark samples agree with Inkmark after entity decoding; generated GFM has unequal output.

### Established workloads

| Workload | #52 ms | Candidate ms | Inkmark ms | Time vs. #52 |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013310 | 0.013521 | 0.019388 | +1.6% |
| mixed-100k | 0.396047 | 0.396697 | 0.412547 | +0.2% |
| mixed-1m | 4.004868 | 3.982911 | 4.422400 | -0.5% |
| gfm-100k | 0.610568 | 0.608064 | 0.714119 | -0.4% |
| references | 0.554204 | 0.551015 | 0.850550 | -0.6% |
| long-lines-1m | 0.110477 | 0.110298 | 0.587042 | -0.2% |
| code-1m | 0.071537 | 0.070573 | 0.322235 | -1.3% |
| definitions-256 | 0.039112 | 0.038882 | 0.043432 | -0.6% |
| definitions-1024 | 0.154530 | 0.152918 | 0.168644 | -1.0% |
| definitions-4096 | 0.669817 | 0.662191 | 0.696772 | -1.1% |
| multiline-small | 0.001505 | 0.001511 | 0.002508 | +0.4% |
| multiline-100k | 1.046888 | 1.033733 | 1.211000 | -1.3% |

The original four range from 0.5% less time to 1.6% more time versus #52. These small changes are within the scale of observed process variability; this study does not claim a general parser or event-delivery improvement.

### Key-shape controls

| Workload | #52 ms | Candidate ms | Inkmark ms | Time vs. #52 |
| --- | ---: | ---: | ---: | ---: |
| keys-uppercase-1024 | 0.137554 | 0.135025 | 0.146258 | -1.8% |
| keys-unicode-1024 | 0.195017 | 0.174289 | 0.191515 | -10.6% |
| keys-spaced-1024 | 0.157541 | 0.153254 | 0.190974 | -2.7% |
| keys-random-1024 | 0.123364 | 0.122752 | 0.145807 | -0.5% |
| keys-long-uppercase-1024 | 0.621488 | 0.615675 | 4.431636 | -0.9% |
| keys-long-spaced-1024 | 0.638657 | 0.641015 | 4.518375 | +0.4% |
| keys-unicode-dense-1024 | 0.400432 | 0.402203 | 0.387702 | +0.4% |
| keys-unicode-long-1024 | 4.327887 | 4.362765 | 5.769630 | +0.8% |
| keys-unicode-upper-long-1024 | 5.111457 | 5.148956 | 5.341000 | +0.7% |
| keys-unicode-lower-long-1024 | 4.367629 | 2.911232 | 5.364138 | -33.3% |
| keys-unicode-upper-tail-1024 | 4.406548 | 4.739233 | 5.515852 | +7.5% |

The original `clé-N` control improves by 10.6%, and the long lowercase Unicode control by 33.3%. Mixed and all-uppercase long Unicode are close to unchanged (+0.8% and +0.7%). The **late-uppercase control regresses 7.5%**, with no allocation reduction. Its labels repeat lowercase Greek/Latin/CJK text and place a capital sigma at the end, forcing the complete predicate scan before both conversions. This is the known cost of the retained design.

Spaced ASCII labels have a 2.7% lower median in this cohort, but use the unchanged ASCII implementation and have 3–4% process spread. This does not establish recovery of the earlier study's repeated-space regression against #51.

### Real documents

| Workload | #52 ms | Candidate ms | Inkmark ms | Time vs. #52 |
| --- | ---: | ---: | ---: | ---: |
| rust-readme | 0.009445 | 0.009471 | 0.015936 | +0.3% |
| clap-readme | 0.006587 | 0.006740 | 0.010916 | +2.3% |
| tokio-changelog | 1.040345 | 1.041445 | 1.116348 | +0.1% |
| rust-nll-rfc | 0.179982 | 0.179988 | 0.249152 | +0.0% |
| awesome-rust-readme | 1.202659 | 1.251411 | 1.552024 | +4.1% |
| commonmark-spec | 0.471466 | 0.483677 | 0.503443 | +2.6% |

These are complete, unmodified public documents: Rust and Clap READMEs, Tokio's changelog, Rust RFC 2094, Awesome Rust's large README, and the CommonMark 0.31.2 specification source. Five use the GFM profile with footnotes disabled; the specification uses CommonMark. [The manifest](../../real-documents.json) pins source revisions/version, byte sizes, SHA-256 hashes, attribution and licenses. Sources are fetched into ignored data, never updated implicitly. The CommonMark specification is by John MacFarlane and contributors under CC-BY-SA-4.0; repository source/license links are recorded for the other documents.

All six real documents agree with Inkmark after entity decoding; Clap also agrees byte for byte. They all retain lower medians than Inkmark, but several have higher medians than our own #52 baseline. The paired repeat below preserves that unfavorable evidence.

## Paired real-document repeat and variability

A separate three-round paired repeat uses the same frozen binaries, without Inkmark, to investigate the real-document differences. It is not pooled with or substituted for the primary cohort. [Raw repeat samples](real-repeat.jsonl) and [summary](real-repeat-summary.json) retain every result.

| Workload | Primary time vs. #52 | Paired repeat time vs. #52 |
| --- | ---: | ---: |
| rust-readme | +0.3% | -0.3% |
| clap-readme | +2.3% | +5.2% |
| tokio-changelog | +0.1% | -0.1% |
| rust-nll-rfc | +0.0% | +0.7% |
| awesome-rust-readme | +4.1% | +2.8% |
| commonmark-spec | +2.6% | +1.4% |

Clap and Awesome Rust retain slower medians in both cohorts. These observed differences must not be described as proven neutral or dismissed as noise. This experiment does not isolate their causal cost from code placement and host/process variability.

An attempted `inline(never)` annotation generated **identical machine-code bytes** for the entire `__text` section and identical disassembly; both callers already call the helper out of line. It was discarded. Its two-round screening remains separately as a code-equivalent variability control: medians differ by up to 4.3% on these real documents despite identical code bytes. [Section hashes](exploratory/outline-sections.json), [timings](exploratory/outline-fold.jsonl) and [summary](exploratory/outline-fold-summary.json) support that limit; they do not erase the primary/repeat differences.

## Allocation and output checks

[Allocation diagnostics](allocations.json) use separate executables with a counting allocator, outside timing. `keys-unicode-1024` avoids 1,027 allocations and 8,129 cumulatively requested bytes per complete conversion; long lowercase Unicode avoids 1,027 allocations and 455,901 requested bytes. Both avoid 1,024 preparation allocations, plus three during reference lookup. No other timed workload saves allocations. Reallocation counts are unchanged for every recorded operation across all twenty-nine inputs. Event counts, document node counts and 120-byte node size remain unchanged; prepared document traversal still allocates nothing. Requested bytes describe allocation traffic, not peak live memory.

All **8,468 saved corpus HTML/Markdown outputs** and **24,276 diagnostic results** remain byte-identical to #52. Every timed output is byte-identical to #52, with agreement among direct HTML, checked events and document traversal. [Preservation](preservation.json), [output agreement](output-agreement.json), archived probes and [the verifier](verify.py) retain input/output hashes and per-document comparisons.

Inkmark matches twenty of the twenty-nine timed outputs byte for byte, and twenty-eight after entity decoding. **Generated GFM differs structurally:** Inkmark leaves `www.` unlinked and table formatting differs, as in the previous study. The timing table therefore compares matching requested profiles, not identical work on every fixture.

## Rejected experiments

[Exploratory metadata](exploratory/metadata.json) links frozen executable hashes, patches and all raw screening rounds. These smaller, sometimes noisy cohorts are distinct from the primary comparison:

- Removing temporary strings by folding characters into existing storage helped mostly-ASCII Unicode labels but regressed dense/long Unicode. Explicit nested pushes were also slower. Skipping redundant per-character lowercase conversions helped mixed/lowercase Unicode; the all-uppercase control exposed a remaining roughly 36% regression. These implementations were discarded.
- Folding complete words through iterator pipelines also regressed dense/uppercase Unicode and was discarded.
- Resuming normalization at the first changed byte added prefix/suffix machinery without a useful gain; it was discarded.
- Combining ASCII and Unicode into one scalar whitespace scanner nearly doubled long ASCII fallback timings. The existing vectorized ASCII scanner was retained.
- The final string-based helper avoids one lowercase string where possible, preserving Rust's efficient complete string conversions for uppercase Unicode. Its predicate cost is visible on the late-uppercase control.
- An attempted inlining annotation changed no machine-code bytes and was discarded.

Screening includes an unusually noisy string-fold cohort during unrelated host activity. No completed screening samples were removed. A development validation attempt ran out of disk space; only this repository's disposable debug build artifacts were cleaned, then the final validation was rerun successfully. Neither failed build attempts nor diagnostics overlap the primary timing cohort.

## Validation and reproduction

Native coverage passes **13,528/13,528 measured source regions**, excluding 22 invariant-only unreachable regions. Default workspace tests, all-feature/all-target Clippy, formatting, and x86_64 Linux all-feature library compilation pass. Linux is a compile-only check here; native runtime coverage is AArch64. [Validation metadata](validation.json) identifies the source and configured checks. Independent source review approved `7663402`; the complete evidence head is reviewed before opening the PR.

The primary run has three sequential alternating engine rounds, with 200 ms warmup and seven calibrated approximately 150 ms batches per process. Summaries take the median of three process medians. All ninety process/profile records and 261 document/process results are retained in [raw timing](html.jsonl); [the summary](html-summary.json) preserves every process median and spread. Startup/input loading are excluded; conversion, output disposal, and normal Ruby GC are included. Timing uses the native system allocator. This task's builds, tests, allocation checks and source review finish before the primary run; the shared Apple M4 Pro host also runs unrelated work and has no affinity or full isolation. No primary samples are excluded. The README candidate's three process medians span 39.4%; small differences on ordinary inputs deserve caution.

Use the pinned [comparison setup](../../readme.md), recorded compiler/locks, and immutable executables outside Cargo target directories. The recorded Ruby gems are installed under `/tmp/core-markdown-comparison/gems`; `measure.py` and `verify.py` select that GEM_HOME/GEM_PATH. From the repository root:

```sh
python3 benches/comparison/prepare.py
python3 benches/comparison/prepare_real_documents.py
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/prepare.py
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/measure.py \
  --binary baseline=/tmp/study/baseline --binary candidate=/tmp/study/candidate \
  --binary inkmark=inkmark --work /tmp/study \
  --output /tmp/study/html.jsonl
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py --input /tmp/study/html.jsonl
```

[inputs.json](inputs.json) lists all input/profile pairs. Reused multiline and key-shape inputs are committed in prior studies. The real-document downloader validates hashes and preserves UTF-8 source bytes/line endings; the new control generator is deterministic. Use a new output path for a new cohort; do not append different binary identities to a recorded run. `summarize.py --input PATH` writes a summary beside a fresh result file; with no arguments it only recomputes recorded cohorts, including screening. The paired repeat uses only baseline/candidate with `--input gfm=.../data/bench-real-gfm.json --input commonmark=.../data/bench-real-commonmark.json`.

`verify.py --work /tmp/study` expects those frozen binaries plus `baseline-allocations`, `candidate-allocations`, `baseline-driver`, and `candidate-driver`. Build allocation adapters from their matching source revisions; build the [saved diagnostic driver](../2026-10-25-single-parser/probe-driver.rs) against each revision with `serde`. The verifier reuses prepared CommonMark/GFM/custom fixtures and the prior preservation manifest, and reads compressed saved probes in this and the single-parser study. The archived 672-case GFM specification fixture is reused from that earlier comparison, under [the GFM specification's](https://github.github.com/gfm/) CC-BY-SA-4.0 license (GitHub and contributors, based on John MacFarlane's CommonMark specification); the separately downloaded published corpus has 677 cases.
