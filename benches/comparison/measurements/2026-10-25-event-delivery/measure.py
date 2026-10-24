"""Measure frozen adapters sequentially; preserve every calibrated sample."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--binary', action='append', required=True, help='LABEL=/absolute/path or inkmark=inkmark')
parser.add_argument('--rounds', type=int, default=3)
parser.add_argument('--suite', choices=['html', 'phases'], default='html')
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
study = Path(__file__).resolve().parent
comparison = study.parents[1]
binaries = dict(value.split('=', 1) for value in args.binary)
args.work.mkdir(parents=True, exist_ok=True)
launch = args.work / 'run'
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

inputs = [('commonmark', comparison / 'data/bench-commonmark.json'),
          ('gfm', comparison / 'data/bench-gfm.json')]
if args.suite == 'html':
    inputs += [('commonmark', comparison / 'data' / name) for name in
               ['bench-references.json', 'bench-scanners.json', 'bench-definitions.json']]
    inputs.append(('commonmark', study.parent / '2026-10-25-recover-single-parser/multiline.json'))
    inputs.append(('commonmark', study / 'key-shapes.json'))
modes = ['core-prepare', 'core-events'] if args.suite == 'phases' else ['core-markdown']
saved = [json.loads(line) for line in args.output.read_text().splitlines()] if args.output.exists() else []
for row in saved:
    assert row['binary_sha256'] == (None if row['engine'] == 'inkmark' else sha(binaries[row['engine']]))
completed = {(row['round'], row['engine'], row['mode'], row['profile'], row['input_sha256']) for row in saved}

for round_number in range(1, args.rounds + 1):
    order = list(binaries) if round_number % 2 else list(reversed(binaries))
    for engine in order:
        for mode in modes:
            for profile, path in inputs:
                digest = sha(path)
                if (round_number, engine, mode, profile, digest) in completed:
                    continue
                if engine == 'inkmark':
                    if mode != 'core-markdown':
                        continue
                    command = ['bundle', 'exec', 'ruby', 'adapter.rb', 'inkmark', 'bench', profile, str(path)]
                else:
                    # Fresh inodes avoid stale Mach-O signature state after overwriting an executed file.
                    launch.unlink(missing_ok=True)
                    shutil.copyfile(binaries[engine], launch)
                    launch.chmod(0o755)
                    command = [str(launch), 'bench', mode, profile, str(path)]
                print('START', round_number, engine, mode, path.name, flush=True)
                output = subprocess.check_output(command, cwd=comparison, env=env, text=True)
                row = dict(round=round_number, engine=engine, mode=mode, profile=profile,
                           input=path.name, input_sha256=digest,
                           binary_sha256=None if engine == 'inkmark' else sha(binaries[engine]),
                           timestamp_utc=datetime.now(timezone.utc).isoformat(),
                           results=[json.loads(line) for line in output.splitlines()])
                with args.output.open('a') as file:
                    file.write(json.dumps(row) + '\n')
                print(engine, [(value['document'], value['median_seconds'] * 1000) for value in row['results']], flush=True)
