# Inline destination scanning on real documents

The shared candidate-byte scanner removes per-byte branch work from ordinary bare inline link destinations. The primary three-round comparison against squash-merged #53 takes **14.9% less time on Clap's README** and **18.2% less on Awesome Rust's README**. A separate paired repeat finds **14.2%** and **16.2%** less time respectively. The original four workloads take 0.4–1.8% less time in the primary cohort; these smaller changes should be interpreted against process variability.

Core-markdown has lower medians than Inkmark 0.2.1 on **28/29 inputs**. Dense Unicode reference keys remain **4.1% slower**. The CommonMark specification takes **2.2% more time than #53** in the primary run and **0.2% less** in the separate repeat. Neither result is omitted or pooled with the other. These shared-host results do not establish a universal ranking.

## Implementation

Production commit **`08836d8a3474f220e60c09a90be17d8ae259d07a`**, source tree **`2d832ba5ba6c4dbdae14f11f6a8ca9d65c79479a`**, adds one static candidate set and replaces the bare-destination byte loop's advance with the existing scanner. Only escapes, parentheses, Markdown/ASCII whitespace and control bytes need scalar handling. Ordinary ASCII and UTF-8 bytes can be skipped together. NUL is deliberately excluded from the set because it is accepted as destination data and replaced during value decoding. Escaped punctuation, the 32-level nesting limit, invalid controls, titles and source bounds retain their previous handling.

The existing scanner uses NEON on supported AArch64 targets and its portable scalar path elsewhere. There are no new unsafe operations, parser fields, buffers, dependencies, allocations, backend choices or APIs. Angle-enclosed destinations and titles retain their existing scanners. This small change shares established scanning code rather than introducing another implementation. The source change adds eleven net production lines; the extra static table occupies read-only storage.

## Profiles and scope

Current #53 was profiled on Clap, Tokio, Awesome Rust and mixed-1m. The initial profiles separately sample complete conversion, preparation and event consumption. They are independent complete operations, including disposal, not additive phase measurements. Inlined work remains attributed to its containing symbol: `render_inner` includes event production as well as HTML work, and event-only adapter frames include inlined delivery and cleanup. They must not be described as pure rendering overhead.

A clean paired confirmation samples complete conversion from the frozen baseline and final candidate, outside builds, tests and timing. Destination parsing (`resource`) drops from **17.0% to 2.3%** of exclusive samples on Clap and **19.9% to 4.0%** on Awesome Rust. On mixed-1m it falls from **2.6% to 0.4%**. These sample shares are diagnostic, not elapsed-time speedups; absolute runtime improvement comes from the separate timing cohorts.

[Initial profiles](profiles/initial-summary.json), [paired profiles](profiles/paired/summary.json) and their compressed raw stacks retain the evidence. Tokio instead shows reference-definition parsing and comparisons as prominent work. Mixed-1m is dominated by its combined event/rendering loop and inline preparation. This change addresses the shared cost in link-heavy documents, not every remaining hotspot.

The previous study recorded small real-document regressions after Unicode folding changes. Clap and Awesome Rust use the no-reference inline specialization in these profiles, so the new Unicode normalization helper is not executed on that inline path. This study does not isolate the previous regression's cause from code placement and shared-host variability. It identifies and removes a separately measurable destination-scanning cost; it does not retrospectively erase the previous measurements.

## Complete HTML conversion

Milliseconds per conversion; lower is faster. Baseline is squash-merged #53, **`f2f208378c7094fcf3cc7c1f0f21c3fb0ace57b6`**, using its frozen source-equivalent production adapter. [Metadata](metadata.json) identifies sources, binaries, toolchains, locks and every input hash. The candidate is the production commit above.

| Workload | #53 ms | Candidate ms | Inkmark ms | Time vs. #53 |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013604 | 0.013416 | 0.019405 | -1.4% |
| mixed-100k | 0.398778 | 0.393345 | 0.415127 | -1.4% |
| mixed-1m | 4.001461 | 3.931404 | 4.457829 | -1.8% |
| gfm-100k | 0.623718 | 0.621221 | 0.720311 | -0.4% |
| references | 0.558366 | 0.558165 | 0.864912 | -0.0% |
| long-lines-1m | 0.112923 | 0.113062 | 0.592769 | +0.1% |
| code-1m | 0.072318 | 0.072267 | 0.325261 | -0.1% |
| definitions-256 | 0.039535 | 0.039625 | 0.043568 | +0.2% |
| definitions-1024 | 0.156323 | 0.156552 | 0.171457 | +0.1% |
| definitions-4096 | 0.678813 | 0.681753 | 0.699615 | +0.4% |
| multiline-small | 0.001527 | 0.001553 | 0.002542 | +1.7% |
| multiline-100k | 1.033266 | 1.036137 | 1.229847 | +0.3% |
| keys-uppercase-1024 | 0.137144 | 0.137260 | 0.147329 | +0.1% |
| keys-unicode-1024 | 0.175254 | 0.177029 | 0.193683 | +1.0% |
| keys-spaced-1024 | 0.155777 | 0.157095 | 0.191733 | +0.8% |
| keys-random-1024 | 0.125242 | 0.124816 | 0.148272 | -0.3% |
| keys-long-uppercase-1024 | 0.631801 | 0.643136 | 4.392606 | +1.8% |
| keys-long-spaced-1024 | 0.648778 | 0.660663 | 4.555265 | +1.8% |
| keys-unicode-dense-1024 | 0.411567 | 0.405669 | 0.389804 | -1.4% |
| keys-unicode-long-1024 | 4.412969 | 4.354017 | 5.903308 | -1.3% |
| keys-unicode-upper-long-1024 | 5.219726 | 5.202159 | 5.471821 | -0.3% |
| keys-unicode-lower-long-1024 | 2.978243 | 2.961895 | 5.458000 | -0.5% |
| keys-unicode-upper-tail-1024 | 4.824286 | 4.787656 | 5.493964 | -0.8% |
| rust-readme | 0.009676 | 0.009603 | 0.016249 | -0.8% |
| clap-readme | 0.006697 | 0.005699 | 0.011256 | -14.9% |
| tokio-changelog | 1.062007 | 1.056623 | 1.141054 | -0.5% |
| rust-nll-rfc | 0.182737 | 0.184025 | 0.253563 | +0.7% |
| awesome-rust-readme | 1.243099 | 1.016297 | 1.555821 | -18.2% |
| commonmark-spec | 0.479644 | 0.490247 | 0.512756 | +2.2% |

The primary [raw timing](html.jsonl) contains all 90 process/profile records and 261 document/process results. [The summary](html-summary.json) preserves all three process medians and their spreads. Candidate process spread reaches 56.2% on Awesome Rust, 26.5% on the RFC, 22.5% on Unicode keys and 17.6% on spaced keys. The first-round real-document candidate burst is retained. Small differences elsewhere are not strong evidence of improvement or regression.

## Separate real-document repeat

This three-round baseline/candidate repeat was run after the primary cohort, without Inkmark. It is kept separately in [raw results](real-repeat.jsonl) and [summary](real-repeat-summary.json), never substituted for primary samples.

| Document | Primary time vs. #53 | Repeat time vs. #53 |
| --- | ---: | ---: |
| rust-readme | -0.75% | -0.66% |
| clap-readme | -14.91% | -14.16% |
| tokio-changelog | -0.51% | +0.05% |
| rust-nll-rfc | +0.70% | -1.24% |
| awesome-rust-readme | -18.24% | -16.23% |
| commonmark-spec | +2.21% | -0.21% |

Clap and Awesome Rust improve consistently in the screening, primary and repeat cohorts. The specification's primary slower median does not recur in the repeat; the two cohorts limit the confidence of any small-effect claim. This is not proof that its cost is exactly unchanged.

## Output, allocations and validation

All **8,468 saved corpus HTML/Markdown outputs**, **24,276 existing diagnostic results** and **34,320 additional destination-boundary probes** remain identical to #53. All 29 timed outputs match #53, and direct HTML, checked event rendering and document traversal agree. [Preservation](preservation.json), [destination probes](destination-boundaries.jsonl.gz), [probe results](destination-preservation.json) and [output agreement](output-agreement.json) retain counts and hashes.

The added probes combine prefix lengths 0–64, every ASCII byte plus representative UTF-8 characters, and closed/unclosed/escaped/title suffixes. Integration regressions check vector/tail boundaries, NUL replacement, Unicode, escapes, balanced/unclosed destinations, all controls, and title separation. Existing tests preserve the 32/33 nested-parentheses boundary.

Every allocation count, reallocation count, requested-byte total, event/node count and 120-byte document node size is unchanged across all 29 inputs. [Separate allocator diagnostics](allocations.json) are outside timing; requested bytes measure cumulative traffic, not peak memory. No new zero-allocation claim is implied.

Inkmark agrees on 20/29 outputs byte for byte and 28/29 after entity decoding. All six real documents agree after entity decoding. **Generated GFM has unequal output:** Inkmark leaves `www.` unlinked and table formatting differs. Its timing compares the requested profiles, not identical output work.

Native validation passes **13,535/13,535 measured source regions**, with 22 invariant-only unreachable exclusions, plus default workspace tests, all-feature/all-target Clippy and formatting. Linux x86_64 all-feature library compilation passes as a **compile-only** local check; runtime coverage here is AArch64. [Validation](validation.json) records the commands and source identity. The first coverage attempt overlapped a comment-only edit and could not map source columns; a full rerun on unchanged final source passed. Independent adversarial source review approved the production commit before final timing; the evidence head is reviewed before opening the PR.

## Method and reproduction

Apple M4 Pro, Homebrew Rust 1.99.1/LLVM 23.1.3, Ruby 4.0.9 and pinned Inkmark 0.2.1. Validation uses rustup stable Rust 1.99.1/LLVM 23.1.1. The host is shared without affinity or isolation: an unrelated Fantail observer uses about one core, with Yugabyte and macOS background/security activity also present. Unrelated processes were not stopped. Initial diagnostic profiling partly overlapped this task's release build; paired confirmation profiling and all timing are separate from this task's builds, tests, allocation diagnostics and source review.

Timing uses 200 ms warmup, seven calibrated approximately 150 ms batches per process, and the median of three process medians. Engines run sequentially in alternating order; native launches use fresh inodes. Input loading, process startup and initial options construction are excluded. Conversion, output disposal, system allocation and ordinary Ruby GC are included. Every completed sample is retained. The earlier two-round [screening](exploratory/destinations-screen.jsonl) uses a separate pre-comment binary identified in metadata and is not pooled with final measurements.

Use [the pinned comparison setup](../../readme.md), prepare generated and immutable real inputs, and build frozen native adapters from each recorded source commit outside Cargo target directories. The baseline source tree is identical to the prior study's candidate. [inputs.json](inputs.json) lists all 10 profile/input pairs, including committed controls from earlier studies. This study reuses the previous study's measurement and summary drivers rather than copying them. From the repository root, with frozen binaries under `/tmp/study`:

```sh
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/measure.py \
  --binary baseline=/tmp/study/baseline --binary candidate=/tmp/study/candidate \
  --binary inkmark=inkmark --rounds 3 \
  --inputs benches/comparison/measurements/2026-10-25-inline-destinations/inputs.json \
  --work /tmp/study --output /tmp/study/html.jsonl
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/html.jsonl
python3 benches/comparison/measurements/2026-10-25-inline-destinations/verify.py \
  --work /tmp/study
```

Use new output paths; the measurement driver resumes existing rows and rejects changed binary identities. For the paired repeat omit Inkmark and pass `--input gfm=benches/comparison/data/bench-real-gfm.json --input commonmark=benches/comparison/data/bench-real-commonmark.json`. Verification expects matching `baseline-allocations`, `candidate-allocations`, `baseline-driver` and `candidate-driver`; the driver source and build instructions are in [the prior report](../2026-10-25-normalize-identifiers/readme.md#validation-and-reproduction). [profile.py](profile.py) reproduces individual complete-conversion sampling on macOS; pass each frozen `--binary LABEL=/absolute/path`, plus fresh `--work` and `--output` directories. Run diagnostics and profiling outside timing.

Real-document sources, immutable revisions, attribution, hashes and licenses remain in [the manifest](../../real-documents.json); no source bytes were changed or republished by this study. Reused CommonMark/GFM corpus attribution and fixture hashes remain in the prior reports and preservation manifest.
