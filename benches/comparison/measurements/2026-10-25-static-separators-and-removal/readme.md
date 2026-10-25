# Static separators, borrowed unwrapping, and section-range removal

Three changes remove unnecessary serializer representations, copies and repeated AST edit work. Public APIs, output bytes, feature support and diagnostics remain unchanged. Inkmark was not retimed.

## Changes

1. **Static block separators:** separator policy returns one of three static strings instead of constructing a private `Join` value and interpreting it in a second function. This removes the allocation previously made for every tight-list or adjacent-definition newline. Arbitrary newline counts had no production callers.
2. **Borrowed unwrapping:** text without CR/LF remains borrowed in `LineWrapping::Unwrap`, detected with the existing memchr-based line-ending scanner. Text containing physical line endings retains the original scalar normalization loop. Only adjacent ASCII spaces/tabs are trimmed; consecutive converted breaks still collapse to one space. Unicode whitespace, NUL and trailing horizontal whitespace retain their old behavior. Span-copying alternatives were rejected after they regressed dense short Unicode lines.
3. **Contiguous section removal:** heading selection discovers the predecessor once, and removal splices the selected sibling range once. Each removed root retains its ID and descendants, clears its attachment links and refreshes its own context; the parent item's hints refresh once. Empty removal ranges retain the old no-op behavior. Section copying uses the existing insertion path.

The net production diff is seven fewer lines, excluding tests. The parser, semantic events, HTML writer, SIMD scanners, dependencies and public node layout are unchanged. Section removal adds a small explicit range operation; the serializer changes remove more code than they add.

## Timing

Baseline is squash-merged #56, **`45f9aa3e081fced42b41a64b0f51124bf58249a3`**, source tree **`4b2957eb7d2cca40beb976ed4604ac229806d506`**. Candidate is **`d68784cee65699f690d07f811711c31d2ad1f040`**, source tree **`413a2dc9cabb8d410bd45f4195c4ed00a1a9e330`**. The candidate source was independently approved before final timing.

Tight-list serialization takes **7.1–8.6% less time**. Unwrap takes **4.2–9.8% less time on the original four**, and the plain single-Text control is **5.7× faster**. The largest removal cases fall from **17.17 ms to 0.089 ms**, approximately **194× faster**, including clone/drop.

HTML and ordinary prepared-document Markdown serialization remain within approximately 1.3% of baseline. Paired moves measure **1.1–3.2% slower**, with consistent direction on the two smaller controls; their implementation is unchanged, and this study does not isolate code-layout effects from shared-host effects. Section-copy controls range from 8.0% faster to 2.0% slower. These controls do not establish a general parser/serializer speedup or strict performance neutrality. The accepted changes trade those small measured variations for reduced allocations and much less section-removal work.

Milliseconds per operation; lower is faster. [Raw timing](timing.jsonl) retains all **42 process/mode records and 192 document/process results**. [The summary](timing-summary.json) retains all process medians and spreads; no samples were excluded.

| Operation/workload | #56 ms | Candidate ms | Time change |
| --- | ---: | ---: | ---: |
| html/readme | 0.013400 | 0.013491 | +0.68% |
| html/mixed-100k | 0.398746 | 0.395747 | -0.75% |
| html/mixed-1m | 4.023849 | 3.973843 | -1.24% |
| html/gfm-100k | 0.620527 | 0.617228 | -0.53% |
| markdown/readme | 0.088122 | 0.088695 | +0.65% |
| markdown/mixed-100k | 2.214980 | 2.220345 | +0.24% |
| markdown/mixed-1m | 22.090500 | 22.367559 | +1.25% |
| markdown/gfm-100k | 2.378405 | 2.393319 | +0.63% |
| replace/flat-64 | 0.018878 | 0.017361 | -8.04% |
| replace/flat-256 | 0.070272 | 0.067920 | -3.35% |
| replace/flat-1024 | 0.269088 | 0.269700 | +0.23% |
| replace/nested-32 | 0.032603 | 0.030927 | -5.14% |
| replace/nested-128 | 0.129793 | 0.132330 | +1.95% |
| move/list-64 | 0.004261 | 0.004398 | +3.21% |
| move/list-256 | 0.014210 | 0.014652 | +3.11% |
| move/list-1024 | 0.074181 | 0.075023 | +1.13% |
| join/tight-list-64 | 0.009716 | 0.009028 | -7.08% |
| join/tight-list-256 | 0.038104 | 0.035307 | -7.34% |
| join/tight-list-1024 | 0.155997 | 0.142568 | -8.61% |
| unwrap/readme | 0.099261 | 0.089510 | -9.82% |
| unwrap/mixed-100k | 2.413374 | 2.283780 | -5.37% |
| unwrap/mixed-1m | 24.071440 | 23.066941 | -4.17% |
| unwrap/gfm-100k | 2.569201 | 2.453958 | -4.49% |
| unwrap/plain-100k | 0.101556 | 0.017873 | -82.40% |
| unwrap/multiline-100k | 0.096786 | 0.085675 | -11.48% |
| unwrap/unicode-lines-100k | 0.108681 | 0.101854 | -6.28% |
| unwrap/late-break-100k | 0.101487 | 0.082771 | -18.44% |
| remove/prefix-0-remove-64 | 0.003112 | 0.002822 | -9.32% |
| remove/prefix-64-remove-64 | 0.043932 | 0.003958 | -90.99% |
| remove/prefix-64-remove-1024 | 0.685973 | 0.060046 | -91.25% |
| remove/prefix-1024-remove-1024 | 17.171764 | 0.088716 | -99.48% |
| remove/prefix-1024-remove-1024-to-end | 17.141296 | 0.088114 | -99.49% |

HTML includes complete parsing, rendering and output disposal on the original four inputs. Markdown measures serialization and output disposal of already-parsed documents. Tight-list controls serialize prepared lists of 64/256/1,024 plain items. Unwrap measures prepared-document serialization with `LineWrapping::Unwrap` on the original four plus four synthetic single-Text documents, approximately 100 KB each: plain Unicode, ASCII multiline, Unicode mixed endings, and plain Unicode with a first newline at the end. Those synthetic values keep actual embedded line endings inside Text rather than parsing them into separate events. The late-break case checks the extra detection scan before normalization.

Removal includes cloning a prepared target, removing its selected heading section with an empty replacement, and dropping the result. Target/replacement parsing and heading selection are outside timing. Cases vary retained prefix length (0/64/1,024 paragraphs), removed paragraph count (64/1,024), and an optional retained suffix. Removed paragraphs include emphasis; detached nodes remain in the cloned arena. Clone/drop and all public edit validation are included, so this does not isolate just the pointer splice.

Section-copy and paired-move controls retain the [preceding study's boundaries](../2026-10-25-serializer-and-edits/readme.md#timing): copy includes cloning a prepared target and dropping the result; move shifts a prepared list between quotes and back without cloning or rendering.

Three alternating sequential baseline/candidate rounds run after this task's builds, tests, diagnostics and source review. Each document warms for at least 200 ms, followed by seven batches calibrated to approximately 150 ms. Tables use the median of three process medians. Every process uses a fresh executable inode. Input I/O, JSON and initial options construction are excluded. Timing executables use the native allocator without instrumentation.

Host: Apple M4 Pro, AArch64 macOS, Homebrew Rust 1.99.1/LLVM 23.1.3. The machine is shared, without affinity or isolation; unrelated Fantail, Git/Clippy, Yugabyte and macOS activity was observed. No unrelated processes were stopped. Small effects warrant caution. [Metadata](metadata.json) pins compilers, locks, source trees, input hashes, generated document sizes and frozen executable hashes.

## Allocation and output preservation

[Separate allocation diagnostics](allocations.json) record one operation after caller-owned reusable storage has warmed. Setup and document parsing are excluded except for complete HTML conversion. Result lengths/node counts agree in all 32 records. HTML, section-copy, paired-move and section-removal counts are unchanged; moves allocate nothing, while removal's clone allocates its arena once.

| Serialization | Allocations before | Allocations after | Fewer requested bytes |
| --- | ---: | ---: | ---: |
| Default/readme | 28 | 24 | 4 |
| Default/mixed-100k | 780 | 264 | 516 |
| Default/mixed-1m | 7,707 | 2,573 | 5,134 |
| Default/gfm-100k | 1,080 | 543 | 537 |
| Tight list/64 | 68 | 5 | 63 |
| Tight list/256 | 260 | 5 | 255 |
| Tight list/1,024 | 1,028 | 5 | 1,023 |
| Unwrap/readme | 208 | 24 | 8,681 |
| Unwrap/mixed-100k | 9,294 | 780 | 91,738 |
| Unwrap/mixed-1m | 92,418 | 7,707 | 915,309 |
| Unwrap/gfm-100k | 11,104 | 901 | 81,156 |
| Unwrap/plain-100k | 4 | 3 | 100,000 |
| Unwrap/multiline-100k | 4 | 4 | 0 |
| Unwrap/unicode-lines-100k | 4 | 4 | 0 |
| Unwrap/late-break-100k | 4 | 4 | 0 |

Reallocation counts are unchanged. Requested bytes are cumulative allocation/growth requests, not peak or retained memory. Real multiline text still needs an owned normalized value; ordinary text avoids that copy. Static separators also contribute to the default and unwrap reductions on documents containing tight lists.

[Preservation results](preservation.json) record **8,468 unchanged corpus HTML/Markdown outputs**, **58,596 unchanged diagnostic results**, the original four through direct/checked/document HTML and Markdown serialization, and **120 identical complete edited-arena snapshots**. Diagnostic results compare HTML/Markdown values and errors, not complete serialized event sequences. Snapshots include detached original nodes, source/value representations as serialized, and syntax overrides; both versions deserialize successfully and retain their HTML. Serialization does not distinguish literal from borrowed/owned value storage.

This study additionally compares **4,697 outputs/snapshots**: three tight-list outputs, eight unwrap workloads, five complete removed-section arenas and 4,681 exhaustive unwrap strings of zero through four tokens drawn from ASCII text, Unicode, space, tab, CR, LF, NBSP and NUL. Public regression tests cover retained list tightness/item hints, optional suffixes, detached subtree IDs/descendants, reattachment and serde round trips. Private tests directly check borrowing when text is unchanged. Existing section-flag/no-op tests remain covered.

## Validation and reproduction

Native all-feature coverage passes **13,549/13,549 measured source regions** with 22 invariant-only unreachable exclusions. Default workspace tests, all-feature/all-target Clippy and formatting pass. Linux x86_64 all-feature library compilation passes as a **compile-only** local check; native runtime coverage is AArch64. [Validation metadata](validation.json) records commands, source identity and log hashes. The complete evidence head is independently reviewed before opening the PR.

Use the [comparison setup](../../readme.md) and original fixtures. Generated maintenance fixtures are ignored and deterministically regenerated by [prepare.py](prepare.py); [metadata](metadata.json) retains exact hashes. The original README comes from the established fixture rather than the changing current README. Corpus attribution/licenses remain in the linked original studies; new controls are generated strings.

From the repository root, prepare original fixtures if absent, then use checkouts at the pinned baseline and candidate sources:

```sh
python3 benches/comparison/prepare.py
python3 benches/comparison/measurements/2026-10-25-static-separators-and-removal/prepare.py
python3 benches/comparison/measurements/2026-10-25-static-separators-and-removal/build.py \
  --baseline /path/to/baseline --candidate /path/to/candidate --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-static-separators-and-removal/verify.py --work /tmp/study
python3 benches/comparison/measurements/2026-10-25-static-separators-and-removal/measure.py \
  --work /tmp/study --output /tmp/study/timing.jsonl
python3 benches/comparison/measurements/2026-10-25-normalize-identifiers/summarize.py \
  --input /tmp/study/timing.jsonl
```

`build.py` reuses the preceding maintenance harness's calibration, allocation counter and controls without changing that historical source. It uses the committed diagnostic lockfile and probe driver, and each checkout's comparison lockfile. Maintenance timing/diagnostic adapters enable `serde`; only the separate allocation binary enables its counting allocator. Frozen executables live outside Cargo target directories. Use a fresh output path for a new timing cohort: the driver resumes existing records and rejects changed input/binary identities. Verification also reads archived probes and generated controls from prior studies, prepared by the new generator and comparison setup.

The [exploratory record](exploratory/readme.md) retains the complete initial cohort and both rejected unwrapping variants, their source identities/patches, all raw samples and their separate summaries. They are not pooled with final-source results.
