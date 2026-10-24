"""Run pinned comparison adapters sequentially and retain every raw process result."""
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
parser.add_argument('--suite', choices=['screen', 'html', 'phases', 'control', 'ending', 'ast', 'arena', 'markdown', 'fallback'], required=True)
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
comparison = Path(__file__).resolve().parents[2]
binaries = dict(item.split('=', 1) for item in args.binary)
args.work.mkdir(parents=True, exist_ok=True)
launch = args.work / 'run'
env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

inputs = [
    ('commonmark', comparison / 'data/bench-commonmark.json'),
    ('gfm', comparison / 'data/bench-gfm.json'),
    ('commonmark', comparison / 'data/bench-references.json'),
    ('commonmark', comparison / 'data/bench-scanners.json'),
    ('commonmark', args.work / 'data/entities.json'),
    ('gfm', args.work / 'data/html.json'),
]
if args.suite == 'ending':
    inputs = [inputs[0], inputs[1], inputs[3]]
if args.suite == 'control':
    inputs = inputs[:3]
if args.suite == 'html':
    inputs.append(('commonmark', comparison / 'data/bench-definitions.json'))
if args.suite == 'fallback':
    tasks = [('core-markdown', 'commonmark', args.work / 'data/fallback.json')]
elif args.suite == 'markdown':
    tasks = [('core-markdown-output', profile, path) for profile, path in inputs[:2]]
elif args.suite in ['ast', 'arena']:
    mode = 'core-ast' if args.suite == 'ast' else 'core-arena'
    tasks = [(mode,profile,path) for profile,path in inputs[:2]]
elif args.suite == 'phases':
    tasks = [(mode, profile, path) for profile, path in inputs[:2]
             for mode in ['core-prepare', 'core-events', 'core-stream', 'core-arena']]
else:
    tasks = [('core-markdown', profile, path) for profile, path in inputs]
    if args.suite == 'screen':
        tasks = [task for task in tasks if task[2].name != 'bench-scanners.json']
        tasks += [('core-arena', 'commonmark', inputs[0][1])]

# Appending allows interruption and resumption without replacing any previous sample.
saved = [json.loads(line) for line in args.output.read_text().splitlines()] if args.output.exists() else []
for row in saved:
    assert row['binary_sha256'] == (None if row['engine'] == 'inkmark' else sha(binaries[row['engine']]))
completed = {(r['round'], r['engine'], r['mode'], r['profile'], r['input_sha256']) for r in saved}
for round in range(1, args.rounds + 1):
    order = list(binaries) if round % 2 else list(reversed(binaries))
    for engine in order:
        for mode, profile, path in tasks:
            digest = sha(path)
            if (round, engine, mode, profile, digest) in completed:
                continue
            if engine == 'inkmark':
                if mode != 'core-markdown':
                    continue
                command = ['bundle', 'exec', 'ruby', 'adapter.rb', 'inkmark', 'bench', profile, str(path)]
                binary_sha = None
            else:
                # Use a new inode: overwriting a previously executed Mach-O can leave stale signature state on macOS. Startup is outside the measured batches.
                launch.unlink(missing_ok=True)
                shutil.copyfile(binaries[engine], launch)
                launch.chmod(0o755)
                command = [str(launch), 'bench', mode, profile, str(path)]
                binary_sha = sha(binaries[engine])
            print(f'START {round} {engine} {mode} {path.name}', flush=True)
            output = subprocess.check_output(command, cwd=comparison, env=env, text=True)
            row = dict(round=round, engine=engine, mode=mode, profile=profile,
                       input=str(path), input_sha256=digest, binary_sha256=binary_sha,
                       command=command, timestamp_utc=datetime.now(timezone.utc).isoformat(),
                       results=[json.loads(line) for line in output.splitlines()])
            with args.output.open('a') as file:
                file.write(json.dumps(row) + '\n')
            for value in row['results']:
                print(engine, value['document'], value['median_seconds'] * 1000, flush=True)
