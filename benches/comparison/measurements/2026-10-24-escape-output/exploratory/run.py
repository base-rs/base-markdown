"""Screen one renderer experiment against merged main; keep every sample."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys

out = Path(__file__).resolve().parent
comparison = out.parents[2]
work = Path('/tmp/core-markdown-escape-output')
stage = sys.argv[1]
assert not (out / f'{stage}-binaries.json').exists(), 'Use a new stage name; retain previous runs.'
binaries = {'baseline': work / 'baseline',
            'candidate': comparison / 'target/release/markdown-comparison'}
renderer = comparison.parent.parent / 'src/event_html_renderer.rs'
assert binaries['candidate'].stat().st_mtime >= renderer.stat().st_mtime, 'Build the candidate before benchmarking.'
launch = work / 'run'
metadata = {engine: hashlib.sha256(binary.read_bytes()).hexdigest()
            for engine, binary in binaries.items()}
(out / f'{stage}-binaries.json').write_text(json.dumps(metadata, indent=2) + '\n')
(out / f'{stage}.patch').write_bytes(subprocess.check_output(
    ['git', 'diff', 'origin/main', '--', 'src'], cwd=comparison))
records = []
for pair in range(2):
    for engine in (['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']):
        launch.unlink(missing_ok=True)
        shutil.copyfile(binaries[engine], launch)
        launch.chmod(0o755)
        for fixture in ['commonmark', 'gfm', 'references', 'scanners']:
            profile = 'gfm' if fixture == 'gfm' else 'commonmark'
            command = [str(launch), 'bench', 'core-markdown', profile,
                       str(comparison / f'data/bench-{fixture}.json')]
            raw = subprocess.check_output(command, cwd=comparison, env=os.environ, text=True)
            (out / f'{stage}-{pair}-{engine}-{fixture}.jsonl').write_text(raw)
            for line in raw.splitlines():
                row = json.loads(line)
                records.append(dict(engine=engine, pair=pair, **row))
                print(stage, pair, engine, row['document'], row['median_seconds'] * 1000, flush=True)
summary = {}
for name in dict.fromkeys(row['document'] for row in records):
    summary[name] = {}
    for engine in binaries:
        values = [row['median_seconds'] * 1000 for row in records
                  if row['document'] == name and row['engine'] == engine]
        summary[name][engine] = dict(process_medians_ms=values, median_ms=statistics.median(values))
    summary[name]['time_reduction_percent'] = 100 * (1 - summary[name]['candidate']['median_ms'] / summary[name]['baseline']['median_ms'])
(out / f'{stage}-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2), flush=True)
