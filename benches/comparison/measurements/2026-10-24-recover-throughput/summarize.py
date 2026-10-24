"""Recompute all retained sample medians, including the identical-binary control."""
from collections import defaultdict
import json
from pathlib import Path
import statistics

out = Path(__file__).resolve().parent
for name in ['repeat-control', 'dispatch-control', 'block-dispatch-control', 'html']:
    path = out / f'{name}.jsonl'
    if not path.exists():
        continue
    values = defaultdict(lambda: defaultdict(list))
    seen = set()
    for line in path.read_text().splitlines():
        run = json.loads(line)
        key = run['round'], run['engine'], run['mode'], run['profile'], run['input_sha256']
        assert key not in seen
        seen.add(key)
        for result in run['results']:
            assert len(result['samples_seconds']) == 7
            median = statistics.median(result['samples_seconds'])
            assert abs(median - result['median_seconds']) < 1e-12
            values[result['document']][run['engine']].append(median * 1000)
    summary = {}
    baseline = 'previous' if name == 'repeat-control' else 'current'
    for document, engines in values.items():
        reference = statistics.median(engines[baseline])
        summary[document] = {
            engine: {
                'median_ms': statistics.median(times),
                'process_medians_ms': times,
                'time_change_percent_vs_' + baseline: (statistics.median(times) / reference - 1) * 100,
            }
            for engine, times in engines.items()
        }
    (out / f'{name}-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
