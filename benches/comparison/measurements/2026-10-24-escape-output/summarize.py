"""Recompute phase medians and verify the saved measurement manifest."""
from pathlib import Path
import hashlib
import json
import statistics

out = Path(__file__).resolve().parent
manifest = json.loads((out / 'run-manifest.json').read_text())
phases = {}
for entry in manifest:
    assert hashlib.sha256(Path(entry['command'][-1]).read_bytes()).hexdigest() == entry['input_sha256']
    if not entry['file'].startswith('phase-'):
        continue
    for line in (out / entry['file']).read_text().splitlines():
        row = json.loads(line)
        key = f"{row['document']}/{entry['mode']}"
        phases.setdefault(key, {}).setdefault(entry['engine'], []).append(row['median_seconds'] * 1000)
for engines in phases.values():
    for engine, values in list(engines.items()):
        assert len(values) == 3
        engines[engine] = dict(median_ms=statistics.median(values), process_medians_ms=values)
    engines['time_reduction_percent'] = 100 * (1 - engines['candidate']['median_ms'] / engines['baseline']['median_ms'])
(out / 'phase-summary.json').write_text(json.dumps(phases, indent=2) + '\n')
allocations = []
for fixture in ['commonmark', 'gfm', 'references']:
    records = {}
    for engine in ['baseline', 'candidate']:
        records[engine] = [json.loads(line) for line in (out / f'allocations-{engine}-{fixture}.jsonl').read_text().splitlines()]
    for before, after in zip(records['baseline'], records['candidate']):
        assert before == after, (fixture, before, after)
        allocations.append(after['document'])
(out / 'allocation-agreement.json').write_text(json.dumps(dict(
    documents=allocations, all_records_identical=True,
    includes=['allocation calls', 'reallocations', 'cumulative requested bytes', 'event counts', 'AST node counts', 'AST node size'],
), indent=2) + '\n')
print(json.dumps(phases, indent=2))
print('All allocation and event/node count records unchanged.')
