# Initial stages before the known-run correction

All initial timing is retained here, including slower controls and noisy rounds. Candidate means `a141c028a6e92bfc0b28699d5417257e3ea9c32a`, not the final candidate in the parent report. [Metadata](metadata.json) pins the initial frozen binaries and input hashes; [validation](validation.json) records its checks. The three cohorts contain 36 process/mode records and 240 document/process results.

The initial shared scanner caused table escaping to search for a backtick even when its position was already known. In this cohort, table serialization went from 0.023267 ms after scratch consolidation to 0.025630 ms after scanner consolidation (10.2% more time), although it remained faster than baseline. Commit `c88e46a` shares the run-continuation loop directly at a known start and retains the search iterator for finding later runs. The final cohort measures that source separately. This is an exploratory signal on a shared host, not a precise attribution of every timing difference to that instruction sequence.

Complete prepared-document serializer controls in milliseconds:

| Control | Baseline | Scratch rewrite | Shared scanner | Literal lookahead |
| --- | ---: | ---: | ---: | ---: |
| code-plain | 212.044458 | 212.398000 | 212.988333 | 212.330042 |
| code-marker-runs | 655.129875 | 657.388083 | 655.976917 | 656.664667 |
| code-multiline | 68.527000 | 68.744792 | 68.511125 | 68.632986 |
| math-plain | 170.980625 | 172.020792 | 171.176958 | 172.528000 |
| math-flow-100k | 0.037861 | 0.037689 | 0.015051 | 0.016698 |
| fenced-code-1m | 0.443305 | 0.443520 | 0.021416 | 0.021899 |
| frontmatter-100k | 0.042563 | 0.042583 | 0.034611 | 0.035155 |
| table-code-256 | 0.044019 | 0.023267 | 0.025630 | 0.025620 |
| adjacent-ascii | 75.790250 | 75.872292 | 76.480146 | 76.409563 |
| adjacent-unicode | 109.765834 | 109.087375 | 109.396146 | 109.261834 |
| adjacent-punctuation | 32.119500 | 32.029375 | 31.952008 | 32.138367 |
| code-plain-8 | 0.005752 | 0.005362 | 0.005289 | 0.005439 |
| math-plain-8 | 0.004946 | 0.004603 | 0.004615 | 0.004624 |
| adjacent-ascii-8 | 0.003054 | 0.003053 | 0.003053 | 0.002159 |
| adjacent-unicode-8 | 0.003795 | 0.003801 | 0.003855 | 0.002805 |
| adjacent-punctuation-8 | 0.012930 | 0.013028 | 0.013049 | 0.013373 |

The literal-prefix shortcut reduces short adjacent ASCII/Unicode text medians by 29.3%/26.1%. Plain code/math short controls improve by 5.4%/6.5%; punctuation-heavy short text takes 3.4% more time. The 2,048-node ASCII/Unicode controls remain approximately flat because repeated escaping of growing preceding context dominates these synthetic documents. This pass does not fix that scaling behavior.

Process spreads are retained in the summary files. Examples include 42.7% for the initial candidate code fence, 31.7% for frontmatter, and 75.0% for scanner-stage short Unicode text. No batches or processes were discarded.

- Original four HTML/Markdown: [raw samples](original.jsonl), [summary](original-summary.json).
- Eleven large controls: [raw samples](stages.jsonl), [summary](stages-summary.json).
- Five short controls: [raw samples](small.jsonl), [summary](small-summary.json).
- [Allocation diagnostics](allocations.json) and [preservation results](preservation.json) use the initial candidate. The latter include the later supplemental check of all sixteen controls and 21,082 probes, after double-dollar math probe selection was corrected; timing inputs are unaffected.
