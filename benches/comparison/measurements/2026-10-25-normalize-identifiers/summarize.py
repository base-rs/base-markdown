"""Recompute every sample/process median, retaining screening and repeat cohorts separately."""
from collections import defaultdict
import argparse
import json
from pathlib import Path
import statistics

study = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--input', action='append', type=Path, help='Summarize fresh results instead of recorded cohorts')
args = parser.parse_args()
paths = args.input or [study / 'html.jsonl', study / 'real-repeat.jsonl', *sorted((study / 'exploratory').glob('*.jsonl'))]
for path in paths:
    groups = defaultdict(lambda: defaultdict(list))
    for line in path.read_text().splitlines():
        run = json.loads(line)
        for result in run['results']:
            assert abs(statistics.median(result['samples_seconds']) - result['median_seconds']) < 1e-12
            groups[run['mode'] + '/' + result['document']][run['engine']].append(result['median_seconds'] * 1000)
    summary = {}
    for key, engines in groups.items():
        control = next(name for name in ['baseline', 'candidate', 'unicode-once'] if name in engines)
        baseline = statistics.median(engines[control])
        summary[key] = {engine: dict(median_ms=statistics.median(times), process_medians_ms=times,
            process_spread_percent=(max(times) / min(times) - 1) * 100,
            control_engine=control, time_change_percent=(statistics.median(times) / baseline - 1) * 100)
            for engine, times in engines.items()}
    path.with_name(path.stem + '-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
