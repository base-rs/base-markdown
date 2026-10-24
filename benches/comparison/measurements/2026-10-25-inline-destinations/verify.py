"""Replay established preservation checks and the added destination-boundary probes."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(study.with_name('2026-10-25-normalize-identifiers') / 'verify.py'), '--work', str(args.work)], check=True)
data = gzip.decompress((study / 'destination-boundaries.jsonl.gz').read_bytes())
outputs = [subprocess.check_output([str(args.work / (engine + '-driver'))], input=data) for engine in ['baseline', 'candidate']]
assert outputs[0] == outputs[1]
rows = [json.loads(line) for line in outputs[1].splitlines()]
assert all(not row.get('panic') and row['html'] == row['checked'] == row['arena'] for row in rows)
report = dict(count=len(rows), input_sha256=hashlib.sha256(data).hexdigest(), output_sha256=hashlib.sha256(outputs[1]).hexdigest(), differences=0)
(args.work / 'destination-preservation.json').write_text(json.dumps(report, indent=2) + '\n')
print('Preserved', len(rows), 'additional destination-boundary probes.')
