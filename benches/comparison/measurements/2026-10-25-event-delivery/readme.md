# Source-backed reference keys and separator delivery

Borrowing canonical source-backed reference keys reduces the three established definition-heavy HTML workloads by **9.3–11.7% time versus #51**, with 256–4,096 fewer allocations per conversion. Core-markdown has lower medians than Inkmark on all twelve established workloads in this local run. The new Unicode-key control remains slower than Inkmark, and repeated-space labels regress against #51; the full table retains those limits.

## Implementation

Reference definitions borrow contiguous source labels already in canonical ASCII form. Other labels use owned normalized storage through `Cow<str>`. The existing sorted index and authored definition records remain separate, preserving every definition event and first-definition precedence. The lifetime of borrowed keys is tied to the caller's input buffer and ends with the parser; keys never borrow the owned label or logical inline scratch.

Reference matching folds ASCII letters to lowercase after the existing whitespace collapse and Unicode lower-then-upper fold. The original canonical result contains no lowercase ASCII letters, so this relabeling is one-to-one and preserves equality. The ASCII path folds directly to lowercase; it shares whitespace scanning with the original normalization helper. Footnote keys retain uppercase because they appear in HTML IDs, and the original identifier helper retains its behavior.

Canonical eligibility is checked in one ASCII pass, rejecting altered whitespace, uppercase letters, and non-ASCII bytes before borrowing.

Pending block separators are now delivery phases carrying their source endpoint. This removes a separate `Option<usize>` and avoids checking or clearing it for every inline event. A separator returns delivery to the body phase before further cursor work. Empty regions, unclosed code, final line endings, and budgeted consumption retain their existing semantics. The single parser, semantic event API, source-backed document, and `no_std` support remain unchanged.

## Experiments

Fresh three-round 2×2 controls compare #50, label scanning alone, phase dispatch alone, and merged #51 on mixed-1m HTML and event consumption. They do **not reproduce** the prior study's 1.9% event-consumption slowdown: the merged version's event median is 0.7% below #50 in this run. This does not invalidate the earlier observation or establish either change's isolated causal cost across compiler/process layouts. [Exploratory metadata and raw samples](exploratory/metadata.json) retain the controls and executable identities.

The following experiments were screened separately before selecting the final source:

- Moving the existing separator option check into body delivery helped event-only screening, with small HTML regressions. Replacing that independent option with an explicit delivery phase was the retained state simplification.
- Extracting document completion into a cold helper regressed mixed HTML and event consumption. Separating block delivery into its own function also regressed HTML. Both were discarded.
- Tuple-based index sorting was close to neutral and was discarded. Borrowing canonical keys produced a larger reduction in definition-heavy work without replacing the index.
- The first borrowed-key version applied an extra ASCII case-conversion pass to reference lookups. The retained version shares whitespace scanning and folds ASCII directly to its required case.
- An initial full three-round cohort at `7735e78` exposed a 4.6% regression on repeated-space labels. A single-pass canonical eligibility check then reduced fallback-label overhead in isolated screening. The initial cohort and its summaries are retained separately; the final table measures the revised source.

Screening results are exploratory, use fewer rounds or fewer fixtures, and are distinct from the final comparison. Rejected source patches and every raw screening sample are retained under [exploratory](exploratory/metadata.json). One timing launch was interrupted before workspace-test completion had been confirmed; [its partial records and reason](exploratory/interrupted-overlap.json) are retained separately. The final cohort restarted after validation completed.

## Direct HTML

Milliseconds per conversion; lower is faster. `baseline` is merged #51 (`f0198ff`); `candidate` is production/test commit `62b32ef`. [Metadata](metadata.json) records full source identities, source trees, executable hashes, compiler, host, and lockfiles.

| Workload | #51 ms | Candidate ms | Inkmark ms | Time vs. #51 |
| --- | ---: | ---: | ---: | ---: |
| readme | 0.013809 | 0.013607 | 0.019196 | -1.5% |
| mixed-100k | 0.402614 | 0.401426 | 0.415969 | -0.3% |
| mixed-1m | 4.044864 | 4.024360 | 4.423371 | -0.5% |
| gfm-100k | 0.619273 | 0.614622 | 0.713234 | -0.8% |
| references | 0.574631 | 0.553606 | 0.853419 | -3.7% |
| long-lines-1m | 0.113121 | 0.110946 | 0.588992 | -1.9% |
| code-1m | 0.072332 | 0.071459 | 0.327130 | -1.2% |
| definitions-256 | 0.042687 | 0.038718 | 0.043524 | -9.3% |
| definitions-1024 | 0.174042 | 0.153707 | 0.171980 | -11.7% |
| definitions-4096 | 0.752629 | 0.670433 | 0.708933 | -10.9% |
| multiline-small | 0.001579 | 0.001529 | 0.002551 | -3.2% |
| multiline-100k | 1.065952 | 1.036819 | 1.222577 | -2.7% |
| keys-uppercase-1024 | 0.138015 | 0.138737 | 0.148005 | +0.5% |
| keys-unicode-1024 | 0.199369 | 0.197886 | 0.191751 | -0.7% |
| keys-spaced-1024 | 0.153278 | 0.159327 | 0.192232 | +3.9% |
| keys-random-1024 | 0.137865 | 0.124172 | 0.149199 | -9.9% |

The original four examples take 0.3–1.5% less time than #51; those small differences should be treated cautiously. Definition-heavy examples take 5.4–11.0% less time than Inkmark in this run. Across all sixteen fixtures, core-markdown has fifteen lower medians. **The Unicode-key control takes 3.2% more time than Inkmark. Repeated-space labels take 4.0% more time than #51**, while uppercase keys are essentially unchanged (+0.5%) and canonical randomized keys improve by 9.9%. The eligibility check has a cost when a label requires normalization, even though it can eliminate allocation for canonical source labels.

**GFM has unequal output:** Inkmark leaves the workload's `www.` address unlinked and formats tables differently. The original three CommonMark examples match Inkmark after entity decoding. All other timed CommonMark fixtures match byte for byte, including the key-shape controls. All sixteen baseline/candidate outputs are byte-identical. [Output agreement](output-agreement.json) records each comparison and output hash.

[Raw timings](html.jsonl) retain every calibrated sample; [the summary](html-summary.json) recomputes sample medians and the median of three process medians. Engines run sequentially in alternating order. Each process warms for 200 ms, then runs seven approximately 150 ms batches. Startup and input loading are excluded; conversion and disposal, including normal Ruby garbage collection, are included. The Apple M4 Pro host is shared, without CPU affinity. No local builds, tests, independent review, profiling, or allocation diagnostics overlap final timing. No final samples are excluded. Small differences remain sensitive to background activity and compiler/process layout.

The key-shape controls contain 1,024 shuffled definitions each. Uppercase, Unicode, and repeated-space labels require owned normalization; ordinary randomized lowercase labels can be borrowed. They prevent generalizing only from workloads dominated by canonical ASCII labels. [Inputs](key-shapes.json) and [their generator](prepare.py) retain this distribution.

## Event consumption and allocations

Separate [phase diagnostics](phases.jsonl) measure complete prepare-and-drop and prepare-plus-event-consumption operations, including disposal. They use three alternating rounds, without HTML rendering. These operations are independent, not additive phases or a formal attribution of individual changes. [The phase summary](phases-summary.json) retains every process median.

Preparation ranges from −2.5% to +0.4% time versus #51 on the original four inputs. Prepare-plus-event-consumption ranges from −0.9% to effectively unchanged (−0.02%). These small diagnostic differences do not establish a broad event-production speedup.

The largest definition workload avoids **4,096 allocations** in preparation and complete conversion. README avoids four; the fallback/multiline controls avoid one each. Event counts, document node counts, and node size remain unchanged across all twelve previously recorded allocation workloads. [Allocation records](allocations.json) retain per-operation deltas and complete results, plus standalone key-shape controls. Reallocation counts are unchanged on the paired workloads. Cumulative requested bytes measure allocation traffic, not peak live memory; prepared document traversal still allocates nothing.

## Validation and reproduction

All **8,468 saved corpus HTML/Markdown outputs** and **24,276 diagnostic results** remain byte-identical to #51, without panics or disagreement among direct HTML, checked events, and document traversal. [Preservation records](preservation.json) identify inputs and output hashes. The [single-parser](../2026-10-25-single-parser/readme.md) and [recovery](../2026-10-25-recover-single-parser/readme.md) studies retain the inputs and diagnostic driver.

Native coverage passes **13,454/13,454 measured source regions**, excluding 22 invariant-only unreachable regions. Default workspace Bake tests, default/all-feature all-target Clippy, formatting, and x86_64 Linux all-feature library compilation pass. The Linux check is compile-only; local runtime coverage exercises AArch64. Focused regressions check borrowed storage, whitespace, Unicode expansion and equivalence, duplicate precedence across borrowed and owned keys, and preservation of all authored definitions. Existing tests cover separators, empty regions, unclosed fences, one-event budgets, and repeated terminal pulls. [Validation metadata](validation.json) identifies the reviewed source.

Use the pinned [comparison setup](../../readme.md), recorded compiler/locks, and immutable executables outside Cargo target directories. The #51 control is the immutable adapter used by its study; its production source tree equals squash-merged #51. From this directory:

```sh
python3 prepare.py
python3 measure.py --binary baseline=/tmp/study/main51 \
  --binary candidate=/tmp/study/candidate --binary inkmark=inkmark \
  --work /tmp/study --output /tmp/study/html.jsonl --rounds 3
python3 measure.py --binary baseline=/tmp/study/main51 \
  --binary candidate=/tmp/study/candidate --suite phases \
  --work /tmp/study --output /tmp/study/phases.jsonl --rounds 3
python3 summarize.py
```

`summarize.py` reads JSONL results beside it; copy new results there or adjust its study path. The multiline input is reused from the recovery study and identified by its input hash in each record.
