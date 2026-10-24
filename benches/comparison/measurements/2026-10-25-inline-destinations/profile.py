"""Sample frozen adapters on individual documents; never run alongside timing."""
import argparse
import gzip
import json
from pathlib import Path
import re
import subprocess
import time

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--binary', action='append', required=True, help='LABEL=/absolute/path')
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
comparison = Path(__file__).resolve().parents[2]
args.work.mkdir(parents=True, exist_ok=True)
args.output.mkdir(parents=True, exist_ok=True)
summary = {}
for label, binary in (item.split('=', 1) for item in args.binary):
    for profile, name, input_name in [
        ('gfm', 'clap-readme', 'bench-real-gfm.json'),
        ('gfm', 'tokio-changelog', 'bench-real-gfm.json'),
        ('gfm', 'awesome-rust-readme', 'bench-real-gfm.json'),
        ('commonmark', 'mixed-1m', 'bench-commonmark.json'),
    ]:
        cases = json.loads((comparison / 'data' / input_name).read_text())
        path = args.work / (name + '.json')
        path.write_text(json.dumps([case for case in cases if case['name'] == name]))
        raw = args.work / (label + '-' + name + '.txt')
        with (args.work / (label + '-' + name + '.log')).open('w') as log:
            process = subprocess.Popen([binary, 'profile', 'core-markdown', profile, str(path)], stdout=log, stderr=log)
            try:
                time.sleep(.5)
                subprocess.run(['/usr/bin/sample', str(process.pid), '3', '1', '-file', str(raw)], check=True, stdout=log, stderr=log)
            finally:
                process.terminate()
                process.wait()
        data = raw.read_text()
        (args.output / (raw.name + '.gz')).write_bytes(gzip.compress(data.encode(), mtime=0))
        samples = int(re.search(r'^\s+(\d+) Thread_', data, re.M)[1])
        flat = data.split('Sort by top of stack, same collapsed (when >= 5):')[1].split('Binary Images:')[0]
        rows = []
        for line in flat.splitlines():
            match = re.match(r'\s+(.*?)\s+\(in .*?\)\s+(\d+)\s*$', line)
            if match:
                symbol, count = match[1], int(match[2])
                rows.append(dict(symbol=symbol, samples=count, percent=count / samples * 100))
        summary[label + '/' + name] = dict(samples=samples, exclusive_top_of_stack=rows)
        print('Sampled', label, name, flush=True)
(args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
