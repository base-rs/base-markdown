"""Check 1 MB preparation under seven inherited-environment layouts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
comparison = Path(__file__).resolve().parents[2]
case = next(row for row in json.loads((comparison / 'data/bench-commonmark.json').read_text()) if row['name'] == 'mixed-1m')
source = args.work / 'data/layout.json'
source.write_text(json.dumps([case]))
launch = args.work / 'run'
records = []
for i, padding in enumerate([None, 0, 1, 15, 31, 63, 127]):
    env = dict(os.environ, GEM_HOME='/tmp/core-markdown-comparison/gems', GEM_PATH='/tmp/core-markdown-comparison/gems')
    if padding is not None:
        env['MARKDOWN_BENCH_PADDING'] = 'x' * padding
    order = [('baseline', args.baseline), ('final', args.candidate)]
    if i % 2:
        order.reverse()
    for engine, binary in order:
        shutil.copyfile(binary, launch)
        launch.chmod(0o755)
        command = [str(launch), 'bench', 'core-prepare', 'commonmark', str(source)]
        raw = subprocess.check_output(command, cwd=comparison, env=env, text=True)
        result = json.loads(raw)
        records.append(dict(engine=engine, padding_bytes=padding, command=command,
                            input_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                            markdown_sha256=hashlib.sha256(case['markdown'].encode()).hexdigest(),
                            binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), result=result))
        args.output.write_text(json.dumps(records, indent=2) + '\n')
        print(padding, engine, result['median_seconds'] * 1000, flush=True)
