"""Replay established diagnostics and compare delimiter/footnote controls."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
s = Path(__file__).resolve().parent
w = args.work.resolve()
previous = s.with_name('2026-10-25-footnotes-indexes-and-escape-context')
subprocess.run([sys.executable, str(s.with_name('2026-10-25-serializer-and-edits') / 'verify.py'),
    '--work', str(w)], check=True)
report = json.loads((w / 'preservation.json').read_text())
allocations = json.loads((w / 'allocations.json').read_text())
report['additional_outputs'] = []
inputs = [(previous / name, 'controls', name != 'probes.json')
    for name in ['controls.json', 'fence-controls.json', 'probes.json']]
inputs += [(s / 'delimiter-controls.json', 'controls', True),
    (s / 'delimiter-probes.json', 'controls', False), (s / 'features.json', 'features', True),
    (s / 'sparse-calls.json', 'features', True)]
for path, mode, count_allocations in inputs:
    outputs = [subprocess.check_output([str(w / (engine + '-bench')), 'check', mode, str(path)])
        for engine in ['baseline', 'candidate']]
    assert outputs[0] == outputs[1], str(path)
    report['additional_outputs'].append(dict(input=str(path.relative_to(s.parent)), mode=mode,
        cases=len(outputs[0].splitlines()), input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        output_sha256=hashlib.sha256(outputs[0]).hexdigest()))
    if not count_allocations:
        continue
    counts = {engine: [json.loads(line) for line in subprocess.check_output([
        str(w / (engine + '-allocations')), 'alloc', mode, str(path)]).splitlines()]
        for engine in ['baseline', 'candidate']}
    assert len(counts['baseline']) == len(counts['candidate']) == len(json.loads(path.read_text()))
    for before, after in zip(counts['baseline'], counts['candidate']):
        assert before['document'] == after['document'] and before['counts']['result'] == after['counts']['result']
        allocations.append(dict(mode=mode, document=before['document'], baseline=before['counts'],
            candidate=after['counts'], deltas={key:after['counts'][key] - value
                for key, value in before['counts'].items()}))
(w / 'preservation.json').write_text(json.dumps(report, indent=2) + '\n')
(w / 'allocations.json').write_text(json.dumps(allocations, indent=2) + '\n')
print('Preserved', sum(row['cases'] for row in report['additional_outputs']), 'additional records.')
print('Collected', len(allocations), 'allocation records.')
