# Initial text-coalescing screening

Two alternating baseline/candidate process pairs compare merged main `3f633aba6733e30566ec2b26c01bd683a7075031` with the retained implementation in `7d853c742e2faf4f785246a483a90b5938755791`. Both binaries use the same launch path, arguments, and environment. Each process has 200 ms warmup and seven calibrated samples. Builds, tests, and reviewer CPU work were not concurrent with timing. Machine background activity was not isolated.

Mixed HTML took 1.6% less time at 100 KB and 1.8% less at 1 MB; public event consumption took 2.3% less time at 1 MB. GFM HTML changed by +1.4%; README, references, long prose, and code stayed within 0.4%. These are preliminary screening results, not universal speedup claims. The complete final three-round comparison rechecks all workloads. No screening samples were discarded.

The [summary](summary.json), raw JSONL files, and binary hashes retain all results. The archived driver retains this session's absolute paths and temporary baseline executable. Its one-entry `mixed-1m.json` selects the unchanged mixed 1 MB case from `data/bench-commonmark.json`. Reproduce by rebuilding the recorded revisions and substituting paths while preserving the shared launch path and environment.
