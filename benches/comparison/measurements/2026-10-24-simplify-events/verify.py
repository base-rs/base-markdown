"""Compare complete saved-corpus HTML through all three event/arena paths."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(__doc__)
parser.add_argument('--binary', action='append', required=True, help='LABEL=/absolute/path; baseline first')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
comparison = Path(__file__).resolve().parents[2]
cases = [('commonmark', 'commonmark', comparison / 'data/commonmark.json'),
         ('gfm-official', 'gfm', comparison / 'data/gfm-official.json'),
         ('gfm-spec', 'gfm', Path('/tmp/core-markdown-comparison/data/gfm-spec.json'))]
cases += [('custom', profile, comparison / 'fixtures/custom.json')
          for profile in ['default', 'commonmark', 'gfm', 'gfm-footnotes']]
outputs = []
expected = {}
for label, binary in (value.split('=', 1) for value in args.binary):
    for corpus, profile, path in cases:
        for engine in ['core-markdown', 'core-stream', 'core-arena']:
            actual = json.loads(subprocess.check_output([binary, 'render', engine, profile, str(path)], cwd=comparison, text=True))
            key = corpus, profile
            if key not in expected:
                expected[key] = actual
            assert actual == expected[key], (label, corpus, profile, engine)
            outputs.append(dict(engine=label, mode=engine, corpus=corpus, profile=profile,
                                outputs=len(actual), input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                output_sha256=hashlib.sha256(json.dumps(actual, ensure_ascii=False).encode()).hexdigest(),
                                byte_identical=True))
args.output.write_text(json.dumps(outputs, indent=2) + '\n')
print('Preserved', sum(len(value) for value in expected.values()), 'outputs in all three paths.', flush=True)
