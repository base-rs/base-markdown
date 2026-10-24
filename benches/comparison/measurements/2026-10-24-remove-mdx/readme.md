# Remove MDX and share parser context

This change removes the MDX recognizers, JSX/expression/ESM AST variants, parser callbacks, dispatch states, source-map helpers, serializer branches, and the SWC test dependency. CommonMark, GFM, frontmatter, math, and the configurable Markdown extensions remain. The implementation loses roughly 5,400 production lines and 9,500 lines overall. The compact parser now shares source and configuration references with its reusable inline workspace instead of retaining duplicate fields. The legacy mdast compiler still retains authored reference syntax and full fence metadata for Markdown serialization; this change does not replace that compiler or migrate its public API.

MDX removal is a breaking API change: `Constructs::mdx`, `ParseOptions::mdx`, their construct fields and callback exports, JSX/MDX AST variants, and hidden identifier/source-map helpers are removed. Braces and JavaScript import/export statements are ordinary Markdown. Angle brackets follow CommonMark HTML rules, including the existing safe-HTML defaults.

Built-in parsing is infallible internally. Public result-returning APIs remain compatible; custom event producers can still return diagnostics, and checked consumers continue validating streams and propagating output failures.

## Measurements

The immutable baseline is merged main `2e4b627`, with the same source tree and binary as the preceding simplification report. The candidate source, executable identities, toolchains, launch conditions, and source tree are recorded in [metadata.json](metadata.json). The harness, fixture bytes, and Inkmark 0.2.1 dependency are unchanged.

| Fixture | Main (ms) | Final (ms) | Inkmark 0.2.1 (ms) | Time change vs main |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013650 | 0.013652 | 0.018886 | +0.0% |
| mixed-100k | 0.397436 | 0.404428 | 0.404217 | +1.8% |
| mixed-1m | 3.955982 | 4.046958 | 4.381889 | +2.3% |
| gfm-100k | 0.601674 | 0.608828 | 0.709805 | +1.2% |
| references | 0.552381 | 0.547817 | 0.855194 | -0.8% |
| long-lines-1m | 0.114580 | 0.114470 | 0.575259 | -0.1% |
| code-1m | 0.069967 | 0.070076 | 0.314156 | +0.2% |
| entities-100k | 0.819269 | 0.837426 | 0.948045 | +2.2% |
| html-safe-100k | 0.253123 | 0.261125 | 0.318866 | +3.2% |
| html-filtered-100k | 0.275410 | 0.277817 | 0.292542 | +0.9% |
| definitions-256 | 0.040636 | 0.040544 | 0.043053 | -0.2% |
| definitions-1024 | 0.166783 | 0.166581 | 0.168044 | -0.1% |
| definitions-4096 | 0.731013 | 0.731650 | 0.692679 | +0.1% |

The initial MDX-only build regressed mixed inputs by about 2–3% against main. Sharing the existing context recovered part of that cost in the controls and removes real duplicate state. The final table reports the actual selected root build; small differences remain sensitive to process layout and machine variation. This is primarily a code-removal change, rather than a new throughput optimization.

[Experiment identities and patches](experiments.json) retain the alternatives and their measured sources. Constructor-adapter changes, private infallible event producers, 16/32/64-byte parser alignment, and local context caching were rejected. The [MDX-only suite](mdx-only-html-summary.json), [constructor](constructor-control-summary.json), [producer](events-control-summary.json), [alignment](alignment-control-summary.json), [shared-context](dedup-control-summary.json), and [cached-context](context-cache-control-summary.json) controls retain every completed sample in their adjacent JSONL files. One shared-context process received SIGKILL before producing results; the run resumed without discarding completed records. Controls are separate runs and their percentages must not be added.

All [2,117 saved corpus outputs](preservation.json) remain byte-identical across direct HTML, checked events, and arena traversal. [Benchmark output agreement](benchmark-output-agreement.json) separately records the 13 timing fixtures. GFM remains an unequal-output comparison: core-markdown links the fixture's `www.example.com`, while Inkmark leaves it literal, and table serialization differs. Entity-decoding agreement is a limited diagnostic rather than a conformance proof.

The [allocation records](allocations.json) have unchanged event/node counts and unchanged allocation/reallocation counts across all 13 fixtures and all measured stages. Cumulative requested bytes are unchanged for parser preparation, event pulling, arena construction/traversal, and HTML rendering. Legacy mdast construction requests 64 fewer bytes per document. Requested bytes include reallocation requests and do not measure peak memory.

The [exact-library size probe](parser-size.json) measures `EventParser` at 1,104 bytes on arm64, down from 1,152; arena nodes remain 120 bytes and the builder remains 64 bytes. The comparison executable is 98,320 bytes smaller; this includes the pinned competitor adapters and is not the size of the library alone.

## Validation and reproduction

Native all-feature/all-target coverage passes 24,480/24,480 measured source regions, with 64 invariant-only `unreachable!` regions excluded. Regression tests preserve ordinary JavaScript-like Markdown, extension boundaries, setext whitespace, invisible AST children, and custom producer diagnostic propagation. Independent source review found no correctness issues. [Validation records](validation.json) identify the checks and logs.

The timing, corpus, allocation, fixture-generation, and size-probe scripts are reused from [the preceding report](../2026-10-24-simplify-events/readme.md). Regenerate the ordinary inputs using `benches/comparison/prepare.py`, and the extra entity/HTML inputs using its `fixtures.py`. Build and retain release executables from both revisions before measuring. From the repository root, the recorded timing command is:

```sh
python3 benches/comparison/measurements/2026-10-24-simplify-events/measure.py \
  --binary baseline=/tmp/core-markdown-simplify-20261024/final \
  --binary candidate=/tmp/core-markdown-unify-20261024/selected \
  --binary inkmark=inkmark --rounds 3 --suite html \
  --work /tmp/core-markdown-simplify-20261024 \
  --output benches/comparison/measurements/2026-10-24-remove-mdx/html.jsonl
```

`measure.py` appends raw results and checks executable hashes when resuming. Use a fresh output file for a fresh experiment. All project builds, tests, profiling, and source review completed before these sequential timing runs. The machine has no CPU affinity or full workload isolation; small timing differences remain sensitive to ordinary machine variation.
