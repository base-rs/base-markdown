"""Record allocation diagnostics separately from timing, including scaling fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--binary', action='append', required=True, help='LABEL=/absolute/path')
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
comparison = Path(__file__).resolve().parents[2]
inputs = [('commonmark', comparison / f'data/bench-{fixture}.json')
          for fixture in ['commonmark', 'references', 'scanners', 'definitions']]
inputs += [('gfm', comparison / 'data/bench-gfm.json'),
           ('gfm', args.work / 'data/html.json')]
inputs += [('commonmark', args.work / f'data/{fixture}.json')
           for fixture in ['entities', 'lists', 'grammar', 'line-ending']]
results = []
for engine, binary in (value.split('=', 1) for value in args.binary):
    for profile, path in inputs:
        command = [binary, profile, str(path)]
        raw = subprocess.check_output(command, cwd=comparison, text=True)
        results.append(dict(engine=engine, profile=profile, command=command,
                            input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                            binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                            results=[json.loads(line) for line in raw.splitlines()]))
args.output.write_text(json.dumps(results, indent=2) + '\n')
