"""Summarize retained process samples and allocation comparisons."""
import argparse
import gzip
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('input', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
contents = args.input.read_bytes()
if args.input.suffix == '.gz':
    contents = gzip.decompress(contents)
data = json.loads(contents)
summary = []
for before, after in zip(data.get('summary', [])[::2], data.get('summary', [])[1::2]):
    assert (before['name'], before['mode']) == (after['name'], after['mode'])
    summary.append(dict(name=before['name'], mode=before['mode'],
        baseline_us=before['median_seconds']*1e6, candidate_us=after['median_seconds']*1e6,
        time_change_percent=(after['ratio_to_baseline']-1)*100,
        baseline_spread_percent=(before['max_seconds']/before['min_seconds']-1)*100,
        candidate_spread_percent=(after['max_seconds']/after['min_seconds']-1)*100))
allocations = []
for before, after in zip(data.get('allocations', [])[::2], data.get('allocations', [])[1::2]):
    assert (before['name'], before['mode']) == (after['name'], after['mode'])
    assert before['counts']['output_bytes'] == after['counts']['output_bytes']
    allocations.append(dict(name=before['name'], mode=before['mode'],
        baseline=before['counts'], candidate=after['counts'],
        deltas={key: after['counts'][key]-value for key,value in before['counts'].items()}))
args.output.write_text(json.dumps(dict(timing=summary, allocations=allocations), indent=2)+'\n')
