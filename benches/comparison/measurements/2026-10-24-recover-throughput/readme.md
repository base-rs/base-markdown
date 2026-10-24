# Recover throughput with simpler block dispatch

Block opening and paragraph interruption now select the applicable scanner from the first content byte. Ordinary text avoids unrelated rule, heading, fence, and list probes. The fence scanner no longer repeats the marker check enforced by both callers. Empty inline delimiter scopes skip the emphasis resolver's setup and scan. No parser state, allocations, public API, feature, dependency, or semantic event representation changes are introduced.

The baseline is merged #42 (`9b707b1`), after MDX removal. Merged #41 (`2e4b627`) separately measures performance before that removal. Retained baseline executables were built before their squash merges from identical source trees; metadata records both build and merged identities. The final library source is `5f3f83e`; [metadata](metadata.json) records exact trees, immutable executables, hashes, toolchains, adapters, and environment.

## Final comparison

Three sequential rounds retain every sample. Entries are medians of three process medians, in milliseconds; negative time changes mean faster rendering.

| Fixture | #41 (ms) | #42 (ms) | Final (ms) | Inkmark 0.2.1 (ms) | Time change vs #42 |
| --- | ---: | ---: | ---: | ---: | ---: |
| readme | 0.013699 | 0.013722 | 0.013620 | 0.018871 | -0.7% |
| mixed-100k | 0.402512 | 0.400754 | 0.399382 | 0.411095 | -0.3% |
| mixed-1m | 4.032753 | 4.078412 | 3.957536 | 4.373371 | -3.0% |
| gfm-100k | 0.608784 | 0.608432 | 0.593759 | 0.705817 | -2.4% |
| references | 0.552582 | 0.549942 | 0.533125 | 0.848202 | -3.1% |
| long-lines-1m | 0.115056 | 0.114474 | 0.113668 | 0.585686 | -0.7% |
| code-1m | 0.070071 | 0.070043 | 0.070426 | 0.317193 | +0.5% |
| entities-100k | 0.827768 | 0.842697 | 0.818353 | 0.957220 | -2.9% |
| html-safe-100k | 0.252664 | 0.256950 | 0.259280 | 0.324151 | +0.9% |
| html-filtered-100k | 0.277303 | 0.278790 | 0.271880 | 0.294375 | -2.5% |
| definitions-256 | 0.040868 | 0.040641 | 0.041366 | 0.042714 | +1.8% |
| definitions-1024 | 0.167271 | 0.167248 | 0.168568 | 0.166845 | +0.8% |
| definitions-4096 | 0.738280 | 0.741865 | 0.739783 | 0.691581 | -0.3% |

Mixed 1 MB takes 3.0% less time than #42 and 1.9% less than #41 in this run. Mixed 100 KB improves only 0.3% against #42, near ordinary variation. This does not recover every fixture uniformly: safe-HTML input still takes 0.9% more time than #42 and 2.6% more than #41; the 256-definition fixture takes 1.8% more than #42. All results remain in the table.

These are local measurements on an Apple M4 Pro Mac. There is no CPU affinity or full machine isolation; small differences can vary with ordinary machine activity and process layout. Parsing, HTML output, and result disposal are timed after warmup; startup, input loading, and options construction are excluded. No project builds, tests, profiling, or independent review ran alongside the timing processes. GFM remains an unequal-output comparison: core-markdown links `www.example.com`, while Inkmark leaves it literal, and table serialization differs. This is not a universal fastest-parser claim.

All [2,117 saved outputs](preservation.json) remain byte-identical across direct HTML, checked events, and arena traversal against both #41 and #42. [Agreement records](benchmark-output-agreement.json) cover all 13 timing fixtures and pinned Inkmark output separately. Entity-decoding agreement is a limited diagnostic, not a conformance proof.

All [allocation records](allocations.json), including allocation/reallocation calls, requested bytes, event counts, and node counts, are exactly unchanged against #42 on every fixture and measured stage. The [size probe](parser-size.json) still measures a 1,104-byte parser, 120-byte arena node, and 64-byte builder on arm64. Requested bytes include growth requests and do not measure peak memory.

## Was the original regression noise?

Before editing Rust, five rounds bracketed the pre-removal executable with two labels for the identical #42 executable. Labels do not affect arguments or environment; all Rust engines use the same launch path. [Raw controls](repeat-control.jsonl) and [recomputed summaries](repeat-control-summary.json) retain every result.

| Fixture | #42 A (ms) | #41 (ms) | Same #42 B (ms) |
| --- | ---: | ---: | ---: |
| readme | 0.014089 | 0.013832 | 0.013917 |
| mixed-100k | 0.409606 | 0.398187 | 0.405886 |
| mixed-1m | 4.075611 | 3.984133 | 4.076232 |
| gfm-100k | 0.607936 | 0.605539 | 0.607426 |
| references | 0.554528 | 0.558676 | 0.552481 |

Both identical-binary aggregate medians were about 2.3% slower than #41 on mixed 1 MB. Their aggregate agreement is not the measurement's precision: individual runs varied by several percent, and the identical-binary 100 KB medians differed by about 0.9%. The controls support a small local mixed regression, with uncertainty in its exact size. They do not establish a portable regression on every machine or a general ranking.

[Five-second sampling profiles](current-profile.txt) against [#41](before-mdx-profile.txt) show time split across event delivery, inline preparation, block preparation, and HTML writes. They do not isolate the precise cause of the original 2% change. This work removes identifiable repeated scanner/resolver work rather than attempting to restore an older compiled layout.

## Controls, validation, and reproduction

The [first control](dispatch-control-summary.json) measures interruption dispatch alone and with empty-scope resolution. The [second control](block-dispatch-control-summary.json) adds block-opening dispatch. [Experiment identities](experiments.json) retain binaries, hashes, source patches, and their applicability to #42. The final source additionally removes the redundant fence-marker guard; it is independently rebuilt and measured in the final comparison.

One process in the second control received SIGKILL without returning a result record; the cause was not determined. The first seven completed records were retained. The harness now unlinks the launch file before copying each executable, avoiding writes to a previously executed inode on macOS. The control resumed under that launcher without discarding completed records. Launch and warmup remain outside timed batches. Historical measurements identify their original script hashes; the final comparison uses the updated script recorded in metadata.

Native all-target/all-feature coverage passes 24,481/24,481 measured source regions, excluding 64 invariant-only unreachable regions. The first coverage run exposed the redundant fence-marker check; it was removed with an explicit scanner precondition, rather than adding a test for an impossible caller. Regression tests cover valid and invalid block markers, ordinary Unicode continuation text, all three line-ending styles, and empty link scopes nested within or preceding emphasis. Independent source review found no correctness issues. [Validation records](validation.json) identify checks, attempts, and log hashes.

Reuse the [fixture generator, corpus verifier, allocation collector, and size probe](../2026-10-24-simplify-events/) from the preceding study. Build release comparison executables for both merged revisions and the candidate and retain them before timing. From the repository root:

```sh
python3 benches/comparison/measurements/2026-10-24-simplify-events/measure.py \
  --binary previous=/tmp/core-markdown-simplify-20261024/final \
  --binary current=/tmp/core-markdown-unify-20261024/selected \
  --binary candidate=/tmp/core-markdown-regain-20261024/final \
  --binary inkmark=inkmark --rounds 3 --suite html \
  --work /tmp/core-markdown-simplify-20261024 \
  --output benches/comparison/measurements/2026-10-24-recover-throughput/html.jsonl
python3 benches/comparison/measurements/2026-10-24-recover-throughput/summarize.py
```

The measurement script appends raw records and verifies executable hashes on resume. Use a fresh output file for a fresh experiment. The original `measure.py` at #42 reproduces the pre-fix A/A and first controls; `experiments.json` identifies the transition in the second control. Input hashes remain unchanged throughout. The separate mdast compiler remains; this optimization does not replace it.
