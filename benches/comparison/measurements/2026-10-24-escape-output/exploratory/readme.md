# Escape-output screening

These are exploratory measurements, not final performance claims. Each complete stage uses two alternating baseline/candidate pairs with the same launch path, arguments, and environment. Baseline is merged main `0e3bdd0`. Source patches, executable hashes, and every sample are retained. Builds, tests, and reviewer workloads did not run concurrently with timing measurements; the machine was not otherwise isolated.

| Stage | Experiment | Decision |
| --- | --- | --- |
| `select-once` | Select the destination once per span and skip empty writes. | No convincing mixed-workload improvement. |
| `inline-local-last` | Inline the wrapper and keep the last byte in a local variable. | Reject: inconsistent mixed gains and slower GFM. |
| `inline-loop` | Inline the replacement loop into destination selection. | Reject: inconsistent mixed-1m results. |
| `compact-result` | Return `fmt::Result` from the replacement helper. | No convincing mixed-workload improvement alone. |
| `clean-span` | Keep unescaped spans on a single-write path and select the destination only when a replacement is found. | Retain for the complete five-run evaluation. |

All patches are cumulative against main; percentages between stages must not be added. `run.py` retains this session's executable paths. Restore the recorded baseline and apply each patch before rebuilding and rerunning under a new stage name.

The `stale-inline-loop-*` records are an aborted launch: a test assertion failed before the release rebuild, leaving the previous candidate executable in place. Only baseline timing records were produced before the driver was stopped. The recorded patch therefore does not describe the candidate binary hash. These incomplete records are retained and excluded from conclusions. The driver now rejects duplicate stage names and candidates older than the renderer source.
