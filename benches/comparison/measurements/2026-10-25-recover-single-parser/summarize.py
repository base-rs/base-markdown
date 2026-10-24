"""Recompute sample medians and summarize each frozen adapter separately."""
from collections import defaultdict
import json
from pathlib import Path
import statistics

study = Path(__file__).resolve().parent
for suite in ['html', 'phases', 'scaling']:
    path = study / f'{suite}.jsonl'
    if not path.exists():
        continue
    groups = defaultdict(lambda: defaultdict(list))
    for line in path.read_text().splitlines():
        run = json.loads(line)
        for result in run['results']:
            assert abs(statistics.median(result['samples_seconds']) - result['median_seconds']) < 1e-12
            groups[run['mode'] + '/' + result['document']][run['engine']].append(result['median_seconds'] * 1000)
    summary = {}
    for key, engines in groups.items():
        summary[key] = {}
        baseline = statistics.median(engines['baseline']) if 'baseline' in engines else None
        for engine, times in engines.items():
            median = statistics.median(times)
            summary[key][engine] = dict(median_ms=median, process_medians_ms=times)
            if baseline:
                summary[key][engine]['time_change_percent'] = (median / baseline - 1) * 100
    (study / f'{suite}-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
