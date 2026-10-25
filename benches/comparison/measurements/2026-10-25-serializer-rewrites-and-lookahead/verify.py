"""Replay corpus diagnostics and compare all four stages on generated serializer cases."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
work = args.work.resolve()
subprocess.run([sys.executable, str(study.with_name('2026-10-25-serializer-and-edits')/'verify.py'), '--work', str(work)], check=True)
report = json.loads((work/'preservation.json').read_text())
report['serializer_stages'] = []
for filename in ['controls.json', 'small-controls.json', 'probes.json']:
    path = study/filename
    outputs = {engine: subprocess.check_output([str(work/(engine+'-bench')), 'check', 'controls', str(path)])
               for engine in ['baseline', 'rewrite', 'scanner', 'candidate']}
    assert all(output == outputs['baseline'] for output in outputs.values()), filename
    report['serializer_stages'].append(dict(input=filename,cases=len(outputs['baseline'].splitlines()),
        stages=list(outputs),input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        output_sha256=hashlib.sha256(outputs['baseline']).hexdigest()))
allocations = json.loads((work/'allocations.json').read_text())
for filename in ['controls.json','small-controls.json']:
    counts = {engine: [json.loads(line) for line in subprocess.check_output([
        str(work/(engine+'-allocations')), 'alloc', 'controls', str(study/filename)]).splitlines()]
        for engine in ['baseline','candidate']}
    assert len(counts['baseline']) == len(counts['candidate']) == len(json.loads((study/filename).read_text()))
    for before, after in zip(counts['baseline'], counts['candidate']):
        assert before['document'] == after['document'] and before['counts']['result'] == after['counts']['result']
        allocations.append(dict(mode='controls',document=before['document'],baseline=before['counts'],candidate=after['counts'],
            deltas={key:after['counts'][key]-value for key,value in before['counts'].items()}))
(work/'preservation.json').write_text(json.dumps(report,indent=2)+'\n')
(work/'allocations.json').write_text(json.dumps(allocations,indent=2)+'\n')
print('Preserved',sum(row['cases'] for row in report['serializer_stages']),'serializer outputs/diagnostics across all four source stages.')
print('Collected',len(allocations),'allocation records.')
