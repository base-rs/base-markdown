# Line-scan screening

These are exploratory measurements, not final performance claims. Each stage uses two alternating baseline/candidate pairs with identical launch paths and environment. Every sample and executable hash is retained; source patches are cumulative against merged main `ad90098`. No project builds, tests, profiling, or reviewer workloads ran concurrently with timing measurements. The machine was not otherwise isolated.

| Stage | Baseline | Change | Decision |
| --- | --- | --- | --- |
| retained-indent | Merged main. | Retain indentation after list/quote recognition. | Initial runs had substantial timing drift; recheck with root skipping. |
| skip-root | Merged main. | Skip the document root during container scans. | Retain for full evaluation; small consistent screening benefit. |
| cached-containers | Root skipping. | Cache the container scan for a paragraph's terminating line. | Reject: little mixed benefit and regressions elsewhere. |
| skip-leaf | Root skipping. | Exclude the active paragraph/code/HTML leaf from container scans. | Reject: no useful mixed gain and references/GFM slower. |
| interrupt-dispatch | Root skipping. | Select interruption classifiers by first byte. | Reject: no convincing overall improvement. |
| root-and-indent | Root skipping. | Retain block indentation instead of resetting and rescanning it. | Retain for full evaluation; modest mixed improvement. |

All stages preserve the 2,117 saved HTML outputs through direct, checked-event, and arena rendering. These checks do not constitute a full semantic-event comparison; the final change has separate regression and review evidence. summary.json reports process medians and changes relative to each stage's baseline; percentages across stages must not be added. quick.py retains session-specific temporary paths. Reproduction requires restoring the identified baseline binary and applying each patch to the recorded merged main.
