# Exploratory measurements

These are diagnostic experiments, not the final comparison. Each stage contains two alternating baseline/candidate pairs from `quick.py`, with the same launch path, arguments, and environment. Seven calibrated batch samples are retained per process. Binary hashes identify the compared snapshots; `summary.json` collects every process median. Stages have different baselines, so their percentages must not be added.

| Stage | Change | Baseline | Decision |
| --- | --- | --- | --- |
| `split` | Outline block transitions into a non-inlined helper. | Merged main. | Reject: event draining improved, but references regressed substantially. |
| `prefix` | Replace literal-link string prefixes with small tags. | Split prototype. | Retain representation change; no isolated timing claim. |
| `prefix-only` | Keep prefix tags without outlining blocks. | Merged main. | Retain: smaller reusable records without the reference regression. |
| `neon` | Locate NEON matches with lane masks and scan eight-byte tails. | Prefix tags. | Reject: no convincing workload improvement. |
| `spans` | Inline endpoint lookup and compute a closing endpoint once. | Prefix tags. | Retain: lower public event-draining overhead. |
| `prepare` | Outline only inline preparation. | Prefix tags and span changes. | Reject: HTML slowed. |
| `specialized` | Separate CommonMark/GFM producer instantiations. | Prefix tags and span changes. | Reject: no useful HTML gain. |
| `hoist` | Select the producer outside the internal rendering loop. | Prefix tags and span changes. | Reject: HTML slowed substantially. |
| `hoist-inline` | Force the hoisted consumption boundary to inline. | Prefix tags and span changes. | Reject: still slower. |
| `inline-escape` | Allow inlining of the HTML escape helper. | Prefix tags and span changes. | Reject: small mixed changes did not establish an overall benefit. |
| `reserve` | Reserve the source length for direct HTML output. | Prefix tags and span changes. | Reject after full evaluation: fewer growth operations, but repeatable long-prose regression. |
| `decode-scan` | Skip ordinary destination/title bytes with `memchr2`. | Prefix/span changes and full-source reservation. | Reject: no improvement in the measured workloads. |
| `reserve-extra` | Reserve 1.5 times the source length. | Source-length reservation. | Reject: no useful gain and more initial spare capacity. |

The final `reserve-capped` stage caps the initial reservation at 4 KiB and includes the scanner fixtures; it is retained. The complete uncapped evaluation and repeated scanner isolation are archived in `../uncapped-output/`.

Rejected prototypes are not production changes. There is no new SIMD implementation, producer abstraction, public API, feature removal, or runtime dependency in this PR. Small timing differences remain sensitive to compiler and process layout; use the final alternating measurements for the retained changes as a whole.
