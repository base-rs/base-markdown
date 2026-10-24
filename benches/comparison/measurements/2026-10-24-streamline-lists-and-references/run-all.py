"""Run the isolated timing matrix sequentially after validation/review have finished."""
import argparse
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--work', required=True, type=Path)
args = parser.parse_args()
out = Path(__file__).resolve().parent
matrix = [
    ('html', ['baseline', 'final', 'inkmark']),
    ('markdown', ['baseline', 'lists', 'final']),
    ('lists', ['baseline', 'lists', 'final']),
    ('grammar', ['baseline', 'references', 'pre-removal', 'final', 'inkmark']),
    ('line-ending', ['baseline', 'references', 'workspace', 'pre-removal', 'final']),
    ('arena', ['baseline', 'final']),
    ('entities', ['baseline', 'lists', 'final']),
]
for suite, engines in matrix:
    command = [sys.executable, str(out / 'measure.py'), '--work', str(args.work),
               '--suite', suite, '--rounds', '3', '--output', str(out / f'{suite}.jsonl')]
    for engine in engines:
        command += ['--binary', f'{engine}=' + ('inkmark' if engine == 'inkmark' else str(args.work / engine))]
    subprocess.run(command, check=True)
subprocess.run([sys.executable, str(out / 'summarize.py')], check=True)
