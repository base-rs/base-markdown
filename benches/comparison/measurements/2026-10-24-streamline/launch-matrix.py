"""Diagnostic: vary launch path and the benchmark's two gem environment entries."""
from pathlib import Path
import hashlib
import json
import os
import resource
import shutil
import subprocess
import time
import sys

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-streamline-20261024')
variant = sys.argv[1] if len(sys.argv) > 1 else 'final'
dest = out / ('screening/launch-matrix' if variant == 'final' else f'screening/launch-matrix-{variant}')
location_filter = sys.argv[2] if len(sys.argv) > 2 else None
if location_filter:
    dest = dest.with_name(dest.name + '-one-path')
dest.mkdir(exist_ok=False)
manifest = []
for location, gem_env in [('screen-run', False), ('screen-run', True), ('final-runs/run', True), ('final-runs/run', False)]:
    if location_filter and location != location_filter:
        continue
    launch = work / location
    env = dict(os.environ)
    for key in ['GEM_HOME', 'GEM_PATH']:
        env.pop(key, None)
        if gem_env:
            env[key] = '/tmp/core-markdown-comparison/gems'
    for engine in ['baseline', 'candidate']:
        binary = work / (variant if engine == 'candidate' else 'baseline')
        launch.unlink(missing_ok=True)
        shutil.copyfile(binary, launch)
        launch.chmod(0o755)
        path = comparison / 'data/bench-commonmark.json'
        command = [str(launch), 'bench', 'core-prepare', 'commonmark', str(path)]
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.monotonic()
        raw = subprocess.check_output(command, cwd=comparison, env=env, text=True)
        wall = time.monotonic() - start
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
        name = f'{location.replace("/", "-")}-gems-{gem_env}-{engine}.jsonl'
        (dest / name).write_text(raw)
        row = dict(file=name, engine=engine, gem_environment=gem_env, command=command,
                   binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                   process_wall_seconds=wall, process_cpu_seconds=cpu)
        manifest.append(row)
        (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        for sample in map(json.loads, raw.splitlines()):
            print(location, gem_env, engine, sample['document'], sample['median_seconds'] * 1000, 'CPU/wall', cpu / wall, flush=True)
