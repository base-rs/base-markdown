"""Check new diagnostic fixtures across all public conversion modes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', required=True, type=Path)
args = parser.parse_args()
out = Path(__file__).resolve().parent
records = []
for fixture in ['lists', 'grammar', 'line-ending']:
    path = args.work / f'data/{fixture}.json'
    cases = json.loads(path.read_text())
    for mode in ['core-markdown', 'core-stream', 'core-arena', 'core-markdown-output']:
        outputs = [json.loads(subprocess.check_output([
            str(args.work / label), 'render', mode, 'commonmark', str(path)], text=True))
            for label in ['baseline', 'final']]
        assert len(outputs[0]) == len(outputs[1]) == len(cases)
        assert outputs[0] == outputs[1], (fixture, mode)
        records.append(dict(fixture=fixture, mode=mode, count=len(cases),
                            input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                            output_sha256=hashlib.sha256(json.dumps(outputs[1], ensure_ascii=False).encode()).hexdigest()))
(out / 'diagnostic-preservation.json').write_text(json.dumps(records, indent=2) + '\n')
print('Preserved', sum(row['count'] for row in records), 'new diagnostic outputs.')
