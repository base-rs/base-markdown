# Streamline serialization, inline storage, and diagnostics

This pass replaces simple serializer regexes with byte scanners, writes Markdown recursively into one output string with reusable scratch, separates compact inline storage from grammar fallback, and removes unist-era diagnostics/location utilities. It removes **161 net lines under `src/`**. The [architecture](../../../../context/markdown-architecture.md) and [diagnostic migration notes](../../../../context/document.md#diagnostics) describe the implementation and public API changes.

The large gain is Markdown serialization: complete parse-plus-Markdown conversion has **5.7–10.1×** the baseline throughput on the original four examples. Direct HTML remains close to baseline, with 1.0–2.1% lower original-four medians. AST-to-HTML medians are 0.1–2.8% higher; this is a measured tradeoff, not an across-the-board speedup.

## Source and method

Baseline is merged main `27f7fa7a2210c3ad34b714500118bf62b64a980d`, production source tree `00589535cfaa5296b4dacf60039345f665531513`. Its immutable adapter was built before production edits, with the same new `core-markdown-output` mode as the final adapter. Final binaries were built at `33a3c3173a089638fc236a8b0288a75bbf9ec80d`, source tree `cd6d2bf72d85de8dc429ecb0978b01e879e1e0aa`. Later evidence-only commits leave that source unchanged. [Metadata](metadata.json) records executable hashes, compiler, host, versions, build command, and background CPU snapshot.

Measurements ran locally on Apple M4 Pro, arm64 macOS, Rust 1.99.1, Ruby 4.0.9, and Inkmark 0.2.1. Each result is the median of three process medians, each retaining seven calibrated samples after 200 ms warmup. Engine order alternates; native binaries launch at the same path with fresh inodes. Input loading, startup, and initial option construction are excluded; conversion, disposal, and ordinary Ruby GC are included. No builds, tests, review, profiling, or allocation diagnostics overlapped timing. This is a shared machine with background processes and no CPU affinity. Small percentage differences should be treated cautiously.

## Markdown output

These are **parse + Document construction + Markdown serialization**, including disposal, not serialization-only timings. Inkmark does not supply a matched Markdown serializer, so it is absent from this table. Milliseconds per complete conversion, lower is faster. [Raw samples](markdown.jsonl), [recomputed summary](markdown-summary.json).

| Workload | Main ms | Cleanup ms | Throughput gain |
| --- | ---: | ---: | ---: |
| readme | 4.200185 | 0.733967 | 5.72× |
| mixed-100k | 99.031646 | 10.287722 | 9.63× |
| mixed-1m | 981.016916 | 99.760250 | 9.83× |
| gfm-100k | 121.495208 | 11.983372 | 10.14× |

The handlers append directly into a shared output buffer. Context ranges replace copied parent strings; scratch is reused for indentation and table escaping. Byte scanners replace repeatedly compiled regexes for physical line endings, backslashes, delimiter lengths, indented-code eligibility, and autolink schemes. Complex configured unsafe patterns still use regexes and remain an allocation-heavy optimization opportunity.

## Direct HTML

[Raw samples](html.jsonl), [recomputed summary](html-summary.json). This is the original four plus nine diagnostic fixtures; lower is faster.

| Workload | Main ms | Cleanup ms | Inkmark ms |
| --- | ---: | ---: | ---: |
| readme | 0.013525 | 0.013323 | 0.018985 |
| mixed-100k | 0.393644 | 0.389795 | 0.409760 |
| mixed-1m | 3.977983 | 3.896204 | 4.390600 |
| gfm-100k | 0.602035 | 0.590843 | 0.717083 |
| references | 0.549299 | 0.526793 | 0.864546 |
| long-lines-1m | 0.113738 | 0.111230 | 0.588124 |
| code-1m | 0.070521 | 0.070413 | 0.325471 |
| entities-100k | 0.835916 | 0.821690 | 0.956887 |
| html-safe-100k | 0.257611 | 0.263320 | 0.320550 |
| html-filtered-100k | 0.277196 | 0.278594 | 0.295961 |
| definitions-256 | 0.040952 | 0.040858 | 0.043254 |
| definitions-1024 | 0.168726 | 0.166222 | 0.167371 |
| definitions-4096 | 0.738260 | 0.730623 | 0.699127 |

The original four medians are ahead of Inkmark in this run. Inkmark remains faster on the largest definition case. Ordinary HTML is 2.2% slower than baseline and filtered HTML is 0.5% slower; the gains elsewhere must not conceal those results.

**GFM is an unequal-output comparison.** Inkmark leaves the fixture's `www.` domain unlinked and differs in table formatting. README/mixed CommonMark results agree after entity decoding, not byte-for-byte. [Output agreement records](benchmark-output-agreement.json) retain every fixture's hashes; these timings do not establish a universal parser ranking or a conformance result.

## AST to HTML

Both adapters build the same source-backed `Document` and use its shared event HTML writer. These timings include parsing, AST construction, rendering, and disposal; they do not isolate the renderer. [Raw samples](arena.jsonl), [recomputed summary](arena-summary.json).

| Workload | Main ms | Cleanup ms | Time change |
| --- | ---: | ---: | ---: |
| readme | 0.027183 | 0.027207 | +0.09% |
| mixed-100k | 0.829145 | 0.852196 | +2.78% |
| mixed-1m | 9.013914 | 9.099696 | +0.95% |
| gfm-100k | 1.276233 | 1.285786 | +0.75% |

The mixed-100KB median is 2.8% higher and the other changes are below 1%. Raw process ranges are retained. We keep this tradeoff visible alongside the much larger Markdown-output gain and smaller parser object.

## Allocation traffic and object sizes

The separate [counting executable results](allocations.json) exclude input loading and initial options. **Serialization alone on an already prepared 1 MB mixed Document** changes as follows:

| Measure | Main | Cleanup | Reduction |
| --- | ---: | ---: | ---: |
| Allocation calls | 20,017,734 | 2,862,476 | 85.7% |
| Reallocations | 2,650,569 | 1,435 | 99.95% |
| Cumulative requested bytes | 5,356,237,063 | 75,100,893 | 98.6% |

Requested bytes measure cumulative allocation/growth traffic, not peak or retained memory. The remaining 2.86 million calls show that this is not allocation-free serialization. Every original diagnostic fixture retains identical direct-HTML and Document-construction allocation counts, requested bytes, and event/node counts; arena traversal still allocates nothing.

[The arm64 layout probe](layout.json) measures `EventParser` shrinking from **1,104 to 368 bytes**. `DocumentNode` remains 120 bytes; `AstBuilder` remains 64 bytes. The compact producer keeps candidates and definitions directly. Its first unsupported inline region transfers those buffers into a reusable boxed grammar workspace. This adds **one allocation requesting 1,024 bytes** when promotion occurs; no second definition table or inline buffer set is created.

A focused forward-reference workload alternates compact and inline-HTML paragraphs, forcing promotion and subsequent reuse. [Raw samples](fallback.jsonl), [summary](fallback-summary.json), and [allocation records](fallback-allocations.json) expose this cost separately:

| Workload | Main ms | Cleanup ms | Inkmark ms |
| --- | ---: | ---: | ---: |
| inline-fallback-small | 0.003278 | 0.003354 | 0.002469 |
| inline-fallback-100k | 2.876521 | 2.870744 | 1.110746 |

The small fixture takes 2.3% more time than baseline; the 100KB fixture is effectively flat. Inkmark is substantially faster on both, so this fallback remains a performance target. Each final conversion records exactly one extra allocation and 1,024 extra requested bytes. [Fixture generation](fixtures.py) is deterministic. Both native adapters produce byte-identical HTML for these transition cases.

## Compatibility and validation

All **8,468 saved outputs** are byte-identical to baseline: 2,117 cases/profile combinations through direct HTML, checked semantic events, arena HTML, and Markdown serialization. [Preservation records](preservation.json) retain counts and input/output hashes. This corpus statement does not claim every possible synthetic AST is unchanged. The scanner deliberately corrects Unicode lookalikes in URL schemes: those now serialize as ordinary resource links rather than invalid autolinks. A public regression covers the correction.

Diagnostics retain `Message` and custom-producer propagation through `StreamError::Parse`. Static reasons use borrowed strings, identifiers are static strings, and an optional byte range replaces boxed unist positions. `message::Place`, public `unist`, and the hidden `Location` helper are removed; see the migration guide.

Native all-feature/all-target coverage passes **21,305/21,305 measured source regions**, excluding 37 invariant-only unreachable regions. Default workspace tests, all-feature and default all-target Clippy, formatting, and Linux all-feature library cross-compilation pass. [Validation records](validation.json) distinguish native runtime checks from Linux compile-only validation. An earlier local disk-space failure was resolved by clearing disposable Cargo artifacts before the recorded complete passing runs. Independent source review approved the measured implementation before timing; its suggested mixed promotion/reuse regression and documentation corrections are included.

## Reproduction

Use [the comparison setup](../../readme.md) and pinned comparison lockfile. Build baseline/final separately and preserve immutable executables outside Cargo's target directories. The baseline needs the same `core-markdown-output` adapter arm. Generate entity/HTML inputs with the preceding study's `fixtures.py WORK/data`; generate transition fixtures with this directory's `fixtures.py WORK/data`.

```sh
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary final=/tmp/study/final --rounds 3 --suite markdown \
  --work /tmp/study --output markdown.jsonl
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary final=/tmp/study/final --binary inkmark=inkmark \
  --rounds 3 --suite html --work /tmp/study --output html.jsonl
python3 summarize.py
```

The driver also supports `--suite arena` and `--suite fallback`. `summarize.py` recomputes all medians from every retained sample; no slow samples are discarded. `preserve.py` saves full outputs and compares immutable adapters. The preceding study's `allocations.py` reproduces the thirteen-fixture counters. `output-agreement.py` retains local immutable-binary paths; substitute equivalent paths when reproducing. Do not overlap timing with other validation workloads.
