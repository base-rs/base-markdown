# Share inline recognition and static serializer escaping

This pass shares inline HTML/autolink recognizers between compact and grammar parsing, and replaces the serializer's dynamic escape engine with fixed scoped byte rules. It removes **688 net lines under `src/`** and makes `regex` a test-only dependency. The [architecture](../../../../context/markdown-architecture.md) describes the paths.

Complete single-line HTML and angle autolinks now retain source-backed compact records. Both producers use the same incremental recognizers; multiline, incomplete, and rejected angles retain grammar handling. The compact loop calls one outlined helper shared across its configurations. Static escape rules preserve consuming-neighbor/non-overlap behavior without constructing pattern vectors, compiling regexes, or allocating regex captures. Existing reusable escape/output scratch remains in place.

## Source and method

Baseline is merged main `f9d2d4850452251b9c3abcb9c45b6e7effd3449a`, production source tree `cd6d2bf72d85de8dc429ecb0978b01e879e1e0aa`. Its immutable binaries are the preceding study's final binaries, with hashes verified against that study's metadata. Final binaries were built from production commit `923ff3d0eece5682fcb5acc99fc07a0db540ac3a`, source tree `fbbb17b4584f9da0537f9020756552f20ee1e0ee`. Subsequent evidence-only commits leave production source unchanged. [Metadata](metadata.json) records exact hashes, executable sizes, compiler, host, versions, and build command.

Measurements ran locally on Apple M4 Pro, arm64 macOS, Rust 1.99.1, Ruby 4.0.9, and Inkmark 0.2.1. Each result is the median of three process medians, each retaining seven calibrated samples after 200 ms warmup. Engine order alternates; native binaries launch at one path with fresh inodes. Input loading, startup, and initial options are excluded; conversion, disposal, and ordinary Ruby GC are included. No local builds, tests, source review, profiling, or allocation diagnostics overlapped timing. This is a shared machine without CPU affinity; background processes and process variation limit conclusions about small percentage changes. All samples and process medians remain available, including slow ones.

The [initial full measurements](initial/metadata.json) precede the helper extraction and are retained separately. The [focused before/after comparison](outline-comparison.jsonl), with [label/source mapping](outline-comparison-metadata.json), recovered 4.7% on long lines by outlining recognition; original-four differences were between -1.0% and +0.5%. The tables below use only the final implementation's complete rerun, without mixing initial samples into the results.

## Direct HTML

Milliseconds per complete conversion; lower is faster. Time change compares cleanup with main. [Raw samples](html.jsonl), [recomputed summary](html-summary.json).

| Workload | Main ms | Cleanup ms | Inkmark ms | Time change |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013575 | 0.013637 | 0.019154 | +0.46% |
| mixed-100k | 0.395935 | 0.397321 | 0.417972 | +0.35% |
| mixed-1m | 3.988011 | 4.008139 | 4.501171 | +0.50% |
| gfm-100k | 0.596841 | 0.597729 | 0.732957 | +0.15% |
| references | 0.535129 | 0.549526 | 0.870524 | +2.69% |
| long-lines-1m | 0.112810 | 0.113530 | 0.602384 | +0.64% |
| code-1m | 0.071065 | 0.071087 | 0.331920 | +0.03% |
| entities-100k | 0.827837 | 0.826492 | 0.981150 | -0.16% |
| html-safe-100k | 0.261072 | 0.265246 | 0.330947 | +1.60% |
| html-filtered-100k | 0.283525 | 0.280684 | 0.300359 | -1.00% |
| definitions-256 | 0.041076 | 0.041448 | 0.043445 | +0.91% |
| definitions-1024 | 0.169655 | 0.171815 | 0.170102 | +1.27% |
| definitions-4096 | 0.743435 | 0.752351 | 0.706579 | +1.20% |

The original four medians are 0.15–0.50% higher than main, effectively close on this machine, and lower than Inkmark in this run. References are 2.7% slower than main, ordinary HTML is 1.6% slower, and filtered HTML is 1.0% faster. Inkmark remains ahead on the two larger definition workloads; the 4,096-definition cleanup median takes 6.5% more time than Inkmark.

**GFM is an unequal-output comparison:** Inkmark leaves the fixture's `www.` domain unlinked and differs in table formatting. The original README/mixed CommonMark results agree after entity decoding, not byte-for-byte. [Output agreement records](benchmark-output-agreement.json) retain every fixture's hashes. These are workload measurements, not a universal parser ranking or conformance claim.

## Inline HTML

The historical `inline-fallback` fixture alternates compact paragraphs, inline HTML, and forward references. It no longer requires inline grammar promotion in the cleanup implementation. [Raw samples](fallback.jsonl), [summary](fallback-summary.json), [allocation records](fallback-allocations.json).

| Workload | Main ms | Cleanup ms | Inkmark ms | Time change |
| --- | ---: | ---: | ---: | ---: |
| inline-fallback-small | 0.003463 | 0.001316 | 0.002483 | -61.99% |
| inline-fallback-100k | 2.968497 | 0.920784 | 1.129504 | -68.98% |

The 100 KB case has **3.22× baseline throughput** and takes **18.5% less time than Inkmark**. Direct HTML allocation calls drop from **27,854 to 10**, reallocations from 2,545 to 12, and cumulative requested bytes from 8,673,356 to 1,834,241. Event counts (45,543), arena node counts (35,422), and output bytes are unchanged. This isolates fallback avoidance; it does not imply every inline HTML form is compact or allocation-free.

## Markdown output

These timings include parsing, Document construction, serialization, and disposal. They do not isolate serialization. Inkmark has no matched Markdown serializer here. [Raw samples](markdown.jsonl), [summary](markdown-summary.json).

| Workload | Main ms | Cleanup ms | Throughput gain |
| --- | ---: | ---: | ---: |
| readme | 0.760853 | 0.109246 | 6.96× |
| mixed-100k | 10.718719 | 2.824652 | 3.79× |
| mixed-1m | 103.663833 | 29.218035 | 3.55× |
| gfm-100k | 12.285997 | 3.321707 | 3.70× |

The four examples gain **3.55–6.96× throughput**. The separate counting executable measures serialization alone on an already prepared 1 MB mixed Document:

| Measure | Main | Cleanup | Reduction |
| --- | ---: | ---: | ---: |
| Allocation calls | 2,862,476 | 20,542 | 99.28% |
| Reallocations | 1,435 | 26 | 98.19% |
| Cumulative requested bytes | 75,100,893 | 2,706,145 | 96.40% |

Requested bytes measure cumulative allocation/growth traffic, not peak memory. [Allocation records](allocations.json) show identical direct-HTML and Document allocation traffic, event/node counts, and allocation-free arena traversal on all thirteen original diagnostics. Serialization still allocates; the figures above do not claim a zero-allocation serializer. The comparison executable shrinks from 5,133,216 to 3,389,360 bytes, but that artifact is not a general library-size guarantee.

## AST to HTML

These timings include parsing, AST construction, rendering, and disposal. [Raw samples](arena.jsonl), [summary](arena-summary.json).

| Workload | Main ms | Cleanup ms | Time change |
| --- | ---: | ---: | ---: |
| readme | 0.027600 | 0.027786 | +0.67% |
| mixed-100k | 0.846761 | 0.865944 | +2.27% |
| mixed-1m | 9.400725 | 9.484591 | +0.89% |
| gfm-100k | 1.301463 | 1.302053 | +0.05% |

AST medians are 0.05–2.27% higher than main. The larger Markdown/HTML-fallback gains should not conceal that tradeoff.

## Compatibility and validation

All **8,468 saved outputs** remain byte-identical to main across direct HTML, checked semantic events, arena HTML, and Markdown serialization. [Preservation records](preservation.json) retain counts and hashes. An additional deterministic 12,000-case valid/malformed angle workload preserves **48,000 outputs** through the same four modes; its [generator](angles-preservation.py) and [records](angles-preservation.json) are retained. Expanded comparison found an interaction between rejected angles and emphasis, so rejected syntax conservatively retains whole-region grammar handling; a minimized public regression covers it.

There is one deliberate formatting correction outside those preserved corpora: ordered-list escape rules now recognize ASCII digits, as Markdown requires. Text such as `١. ordinary` and `١) ordinary` stays unescaped rather than receiving unnecessary backslashes. A public serializer regression covers the correction. No arbitrary public regex customization API is removed; the replaced rules were private and fixed.

Native all-feature/all-target coverage passes **20,979/20,979 measured source regions**, with 37 invariant-only unreachable exclusions. Default workspace tests, default/all-feature all-target Clippy, formatting, and Linux all-feature library cross-compilation pass. [Validation records](validation.json) distinguish native runtime checks from local Linux compile-only checks. Independent adversarial review approved the final production commit after the fallback and helper changes.

Shared reference lookup and a narrower grammar workspace remain separate followups. They are not implemented in this study; single-line HTML now avoids that workspace entirely, making retained multiline/extension workloads important when assessing their value.

## Reproduction

Use the [comparison setup](../../readme.md) and pinned comparison lockfile. Build baseline/final separately and preserve immutable executables outside Cargo targets. Generate entity/HTML inputs with the preceding `2026-10-24-simplify-events/fixtures.py WORK/data`; generate inline-HTML inputs with this directory's `fixtures.py WORK/data`.

```sh
python3 measure.py --binary baseline=/tmp/study/baseline \
  --binary final=/tmp/study/final --binary inkmark=inkmark \
  --rounds 3 --suite html --work /tmp/study --output html.jsonl
python3 summarize.py
python3 angles-preservation.py --work /tmp/study
```

The driver also supports `--suite fallback`, `--suite markdown`, and `--suite arena`. Omit Inkmark for the latter two. `preserve.py` compares exact saved outputs; the preceding study's allocation driver reproduces counters. `output-agreement.py` retains local immutable-binary paths; substitute equivalent paths when reproducing. Do not overlap timing with other validation workloads.
