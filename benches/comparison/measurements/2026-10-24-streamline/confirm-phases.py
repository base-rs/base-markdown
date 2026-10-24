"""Repeat preparation/event diagnostics and retain process CPU/wall accounting."""
from pathlib import Path
import hashlib
import json
import os
import resource
import shutil
import statistics
import subprocess
import time

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-streamline-20261024')
dest = out / 'screening/phase-confirmation'
dest.mkdir(exist_ok=False)
launch = work / 'final-runs/run'
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
manifest, records = [], {}
for pair in range(1, 4):
    for engine in (['baseline', 'candidate'] if pair % 2 else ['candidate', 'baseline']):
        binary = work / ('final' if engine == 'candidate' else 'baseline')
        launch.unlink(missing_ok=True)
        shutil.copyfile(binary, launch)
        launch.chmod(0o755)
        for mode in ['core-prepare', 'core-events']:
            for fixture in ['commonmark', 'gfm']:
                profile = 'gfm' if fixture == 'gfm' else 'commonmark'
                path = comparison / f'data/bench-{fixture}.json'
                command = [str(launch), 'bench', mode, profile, str(path)]
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.monotonic()
                raw = subprocess.check_output(command, cwd=comparison, env=env, text=True)
                wall = time.monotonic() - start
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
                name = f'{pair}-{engine}-{mode}-{fixture}.jsonl'
                (dest / name).write_text(raw)
                manifest.append(dict(file=name, engine=engine, mode=mode, command=command,
                                     input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                     binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                     process_wall_seconds=wall, process_cpu_seconds=cpu))
                (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
                for row in map(json.loads, raw.splitlines()):
                    key = f"{row['document']}/{mode}"
                    records.setdefault(key, {}).setdefault(engine, []).append(row['median_seconds'] * 1000)
                    print(pair, engine, key, row['median_seconds'] * 1000, 'process CPU/wall', cpu / wall, flush=True)
for row in records.values():
    for engine, values in list(row.items()):
        row[engine] = dict(median_ms=statistics.median(values), process_medians_ms=values)
    row['time_reduction_percent'] = 100 * (1 - row['candidate']['median_ms'] / row['baseline']['median_ms'])
(dest / 'summary.json').write_text(json.dumps(records, indent=2) + '\n')
