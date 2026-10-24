"""Centralized sequential worktree screening; never run alongside builds/tests."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-parallel-20261024')
stage = sys.argv[1]
labels = sys.argv[2:]
assert labels
dest = out / 'screening' / stage
dest.mkdir(parents=True, exist_ok=False)
launch = work / 'screen-run'
binaries = {label: work / label for label in ['baseline', *labels]}
metadata = dict(timestamp_utc=datetime.now(timezone.utc).isoformat(),
                binary_sha256={key: hashlib.sha256(value.read_bytes()).hexdigest() for key, value in binaries.items()},
                baseline_commit='4d18a71', method='Two alternating rounds; same Rust launch path, arguments and environment. All samples retained. No project builds/tests/profiles/reviewer workloads overlap timing.')
(dest / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
records, manifest = [], []
for pair in range(2):
    order = list(binaries) if pair == 0 else list(reversed(binaries))
    for label in order:
        launch.unlink(missing_ok=True)
        shutil.copyfile(binaries[label], launch)
        launch.chmod(0o755)
        for fixture in ['commonmark', 'gfm', 'references', 'scanners']:
            profile = 'gfm' if fixture == 'gfm' else 'commonmark'
            input_path = comparison / f'data/bench-{fixture}.json'
            command = [str(launch), 'bench', 'core-markdown', profile, str(input_path)]
            raw = subprocess.check_output(command, cwd=comparison, env=os.environ, text=True)
            filename = f'{pair}-{label}-{fixture}.jsonl'
            (dest / filename).write_text(raw)
            manifest.append(dict(file=filename, label=label, command=command,
                                 input_sha256=hashlib.sha256(input_path.read_bytes()).hexdigest()))
            (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
            for line in raw.splitlines():
                row = json.loads(line)
                records.append(dict(label=label, pair=pair, **row))
                print(stage, pair, label, row['document'], row['median_seconds'] * 1000, flush=True)
summary = {}
for name in dict.fromkeys(row['document'] for row in records):
    summary[name] = {}
    for label in binaries:
        values = [row['median_seconds'] * 1000 for row in records if row['document'] == name and row['label'] == label]
        summary[name][label] = dict(process_medians_ms=values, median_ms=statistics.median(values))
        if label != 'baseline':
            summary[name][label]['time_reduction_percent'] = 100 * (1 - statistics.median(values) / summary[name]['baseline']['median_ms'])
(dest / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2), flush=True)
