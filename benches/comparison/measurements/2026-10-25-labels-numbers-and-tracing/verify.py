"""Compare focused outputs and allocation counts using the established adapters."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
work = args.work.resolve()
report = []
allocations = []
for filename, mode in [('serializer.json', 'controls'), ('numbers.json', 'features')]:
    path = study / filename
    output = {engine: subprocess.check_output([
        str(work / (engine + '-bench')), 'check', mode, str(path)])
        for engine in ['baseline', 'candidate']}
    assert output['baseline'] == output['candidate'], filename
    report.append(dict(input=filename, mode=mode, cases=len(output['baseline'].splitlines()),
        input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        output_sha256=hashlib.sha256(output['baseline']).hexdigest()))
    counts = {engine: [json.loads(line) for line in subprocess.check_output([
        str(work / (engine + '-allocations')), 'alloc', mode, str(path)]).splitlines()]
        for engine in ['baseline', 'candidate']}
    assert len(counts['baseline']) == len(counts['candidate']) == len(json.loads(path.read_text()))
    for before, after in zip(counts['baseline'], counts['candidate']):
        assert before['document'] == after['document']
        assert before['counts']['result'] == after['counts']['result']
        allocations.append(dict(mode=mode, document=before['document'],
            baseline=before['counts'], candidate=after['counts'],
            deltas={key: after['counts'][key] - value for key, value in before['counts'].items()}))
(work / 'focused-preservation.json').write_text(json.dumps(report, indent=2) + '\n')
(work / 'focused-allocations.json').write_text(json.dumps(allocations, indent=2) + '\n')
print('Preserved', sum(row['cases'] for row in report), 'focused outputs.')
print('Collected', len(allocations), 'focused allocation records.')
