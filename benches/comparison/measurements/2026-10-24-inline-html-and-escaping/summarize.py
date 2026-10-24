"""Recompute every process/sample median and summarize alternating comparisons."""
from collections import defaultdict
import json
from pathlib import Path
import statistics

out = Path(__file__).resolve().parent
for suite in ['html', 'arena', 'markdown', 'fallback']:
    path = out / f'{suite}.jsonl'
    if not path.exists():
        continue
    rows = defaultdict(lambda: defaultdict(list))
    for line in path.read_text().splitlines():
        run = json.loads(line)
        for row in run['results']:
            assert abs(statistics.median(row['samples_seconds']) - row['median_seconds']) < 1e-12
            rows[run['mode'] + '/' + row['document']][run['engine']].append(row['median_seconds'] * 1000)
    summary = {}
    for key, engines in rows.items():
        summary[key] = {}
        baseline = statistics.median(engines['baseline'])
        for engine, times in engines.items():
            value = statistics.median(times)
            summary[key][engine] = dict(median_ms=value, process_medians_ms=times,
                                       time_reduction_percent=(1 - value / baseline) * 100)
    (out / f'{suite}-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
