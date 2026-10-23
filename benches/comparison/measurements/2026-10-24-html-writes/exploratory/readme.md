# Renderer experiments

Each stage has two alternating baseline/candidate process pairs. Every process keeps seven calibrated samples after warmup. Both binaries run from the same launch path with identical arguments and environment. Project builds, tests, and reviewer CPU work were not concurrent with timing; machine background activity was not isolated. See `quick.py`, `*-binaries.json`, the raw JSONL files, and [summary](summary.json).

`batched` compares merged main `3750a5289e1998d1eeb4c762f137a376d5af7f28` with retained commit `aa2c98ca57b0aa5f79e3631e1b4695c54d3d2a9c`. All other stages independently compare an uncommitted experiment against that batching commit. Patches record each rejected experiment. The mixed event fixture is the unchanged `mixed-1m` entry from `data/bench-commonmark.json`, selected into a one-entry JSON array.

| Experiment | Mixed 100 KB time change | Mixed 1 MB time change | References time change | Decision |
| --- | ---: | ---: | ---: | --- |
| Batch fixed markup | -0.9% | +0.1% | -4.6% | Keep for final evaluation |
| Skip more private-path structural validation | +1.7% | +0.7% | +2.8% | Reject |
| Skip empty escape writes | +3.9% | +2.9% | +1.0% | Reject |
| Route escaped values to their output once | -0.1% | +0.9% | +0.8% | Reject |

Percentages compare medians of the two process medians; positive is slower. These are screening observations, not precise universal effects. The empty-write run also moved the unchanged public event mode by -2.6%, reinforcing that small differences require caution. Batching's exploratory long-prose change was +2.3%; the final three-round run includes that workload to check it again.

The trusted prototype retained public validation but gated additional checks for the internal parser. It passed the existing focused tests and saved-output comparison, but failed its performance objective. The empty-write and routed-escape prototypes passed the focused event tests. None of these three rejected changes is in production source, and no quote serialization, feature set, or SIMD implementation changed.

The archived driver retains local absolute paths and temporary executables. Reproduce by building the recorded baseline and candidate with the same compiler, substituting their paths, regenerating fixtures with `prepare.py`, and preserving identical launch paths and environment. Stage names must be unique to retain every run.
