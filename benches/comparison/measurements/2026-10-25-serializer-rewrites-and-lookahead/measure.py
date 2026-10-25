"""Alternate frozen executables sequentially, retaining every process and batch sample."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--rounds', type=int, default=3)
parser.add_argument('--mode', action='append', choices=['html', 'markdown', 'controls'])
parser.add_argument('--engine', action='append', choices=['baseline', 'candidate', 'rewrite', 'scanner'])
parser.add_argument('--input', action='append', default=[], help='Override MODE=/path/to/input.json')
args = parser.parse_args()
study = Path(__file__).resolve().parent
work = args.work.resolve()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
inputs = dict(html=study / 'original.json', markdown=study / 'original.json', controls=study / 'controls.json')
for mode, path in (item.split('=',1) for item in args.input):
    assert mode in inputs
    inputs[mode] = Path(path).resolve()
saved = [json.loads(line) for line in args.output.read_text().splitlines()] if args.output.exists() else []
for row in saved:
    assert row['binary_sha256'] == sha(work / (row['engine'] + '-bench'))
    assert row['input_sha256'] == sha(inputs[row['mode']])
completed = {(row['round'],row['engine'],row['mode']) for row in saved}
for round_number in range(1,args.rounds+1):
    engines = args.engine or ['baseline', 'candidate']
    for engine in (engines if round_number % 2 else list(reversed(engines))):
        for mode in args.mode or inputs:
            if (round_number,engine,mode) in completed:
                continue
            binary = work / (engine + '-bench')
            launch = work / 'run'
            launch.unlink(missing_ok=True)
            shutil.copy2(binary,launch)
            command = [str(launch),'bench',mode,str(inputs[mode])]
            print('START',round_number,engine,mode,flush=True)
            output = subprocess.check_output(command,text=True)
            row = dict(round=round_number,engine=engine,mode=mode,input=inputs[mode].name,
                input_sha256=sha(inputs[mode]),binary_sha256=sha(binary),timestamp_utc=datetime.now(timezone.utc).isoformat(),
                results=[json.loads(line) for line in output.splitlines()])
            with args.output.open('a') as file: file.write(json.dumps(row)+'\n')
            print([(r['document'],r['median_seconds']*1000) for r in row['results']],flush=True)
