"""Replay preservation checks and compare the complete serialized event contract."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--events-only', action='store_true', help='Skip already completed output/allocation checks')
args = parser.parse_args()
work = args.work.resolve()
study = Path(__file__).resolve().parent
previous = study.with_name('2026-10-25-inline-destinations')
if not args.events_only:
    subprocess.run([sys.executable, str(previous / 'verify.py'), '--work', str(work)], check=True)
cohorts = [
    study.with_name('2026-10-25-single-parser') / 'cases.jsonl.gz',
    study.with_name('2026-10-25-single-parser') / 'generated-cases.jsonl.gz',
    study.with_name('2026-10-25-normalize-identifiers') / 'label-boundaries.jsonl.gz',
    study.with_name('2026-10-25-normalize-identifiers') / 'boundary-cases.jsonl.gz',
    previous / 'destination-boundaries.jsonl.gz',
]
report = []
for path in cohorts:
    data = gzip.decompress(path.read_bytes())
    outputs = [subprocess.check_output([str(work / (engine + '-events'))], input=data)
               for engine in ['baseline', 'candidate']]
    assert outputs[0] == outputs[1], path.name
    report.append(dict(input=str(path.relative_to(study.parent)), cases=len(data.splitlines()),
                       input_sha256=hashlib.sha256(data).hexdigest(),
                       events_sha256=hashlib.sha256(outputs[1]).hexdigest(), differences=0))
sizes = {engine: int(subprocess.check_output([str(work / (engine + '-events')), 'size']))
         for engine in ['baseline', 'candidate']}
(work / 'event-preservation.json').write_text(json.dumps(dict(cohorts=report, parser_bytes=sizes), indent=2) + '\n')
print('Preserved complete serialized events for', sum(row['cases'] for row in report), 'cases.')
print('Parser sizes:', sizes)
