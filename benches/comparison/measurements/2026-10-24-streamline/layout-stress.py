"""Check scanner preparation under several inherited-environment layouts."""
from pathlib import Path
import hashlib
import json
import os
import resource
import shutil
import subprocess
import time

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-streamline-20261024')
dest = out / 'screening/aligned-layout-stress'
dest.mkdir(exist_ok=False)
case = next(row for row in json.loads((comparison / 'data/bench-commonmark.json').read_text()) if row['name'] == 'mixed-1m')
path = dest / 'input.json'
path.write_text(json.dumps([case]) + '\n')
launch = work / 'final-runs/run'
manifest = []
for padding in [None, 0, 1, 15, 31, 63, 127]:
    env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
    if padding is not None:
        env['MARKDOWN_BENCH_PADDING'] = 'x' * padding
    for engine in ['baseline', 'candidate']:
        binary = work / ('final' if engine == 'candidate' else 'baseline')
        launch.unlink(missing_ok=True)
        shutil.copyfile(binary, launch)
        launch.chmod(0o755)
        command = [str(launch), 'bench', 'core-prepare', 'commonmark', str(path.resolve())]
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.monotonic()
        raw = subprocess.check_output(command, cwd=comparison, env=env, text=True)
        wall = time.monotonic() - start
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        name = f'{padding}-{engine}.jsonl'
        (dest / name).write_text(raw)
        manifest.append(dict(file=name, engine=engine, padding_bytes=padding, command=command,
                             binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                             process_wall_seconds=wall,
                             process_cpu_seconds=after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime))
        (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        print(padding, engine, json.loads(raw)['median_seconds'] * 1000, flush=True)
