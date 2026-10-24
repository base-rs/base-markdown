"""Output preservation and temporary fallback probe; run outside timing windows."""
from pathlib import Path
import collections
import hashlib
import json
import subprocess
import sys

out = Path(__file__).resolve().parent
comparison = out.parents[1]
work = Path('/tmp/core-markdown-parallel-20261024')
old = Path('/tmp/core-markdown-comparison')
cases = [('commonmark', 'commonmark', comparison / 'data/commonmark.json'),
         ('gfm-official', 'gfm', comparison / 'data/gfm-official.json'),
         ('gfm-spec', 'gfm', old / 'data/gfm-spec.json')]
cases += [('custom', profile, comparison / 'fixtures/custom.json')
          for profile in ['default', 'commonmark', 'gfm', 'gfm-footnotes']]
labels = sys.argv[1:] or ['producer-dispatch', 'simd-match', 'uri-runs']
preservation = json.loads((out / 'screening-preservation.json').read_text()) if (out / 'screening-preservation.json').exists() else []
preservation = [row for row in preservation if row['candidate'] not in labels]
for label in labels:
    for name, profile, path in cases:
        expected = json.loads((old / 'results' / f'{name}-{profile}-core-markdown.json').read_text())
        for engine in ['core-markdown', 'core-stream', 'core-arena']:
            actual = json.loads(subprocess.check_output([str(work / label), 'render', engine, profile, str(path)], cwd=comparison, text=True))
            assert actual == expected, (label, name, profile, engine)
        preservation.append(dict(candidate=label, corpus=name, profile=profile,
                                 outputs=len(expected), all_three_paths_byte_identical=True))
(out / 'screening-preservation.json').write_text(json.dumps(preservation, indent=2) + '\n')
print(f'{labels} preserve all 2,117 saved outputs through direct, checked, and arena paths.', flush=True)

inputs = [(row, profile) for fixture, profile in [('commonmark', 'commonmark'), ('gfm', 'gfm')]
          for row in json.loads((comparison / f'data/bench-{fixture}.json').read_text())]
inputs += [({'name': 'diagnostic-inline-html', 'markdown': 'before <span>x</span> after'}, 'commonmark'),
           ({'name': 'diagnostic-block-html', 'markdown': '<custom>\nx\n</custom>'}, 'commonmark'),
           ({'name': 'diagnostic-multiline-code', 'markdown': 'before `a\nb` after'}, 'commonmark'),
           ({'name': 'diagnostic-gfm-path-link', 'markdown': 'Visit https://example.com/path?q=x&v=2'}, 'gfm')]
fallback = []
for row, profile in inputs:
    path = work / 'fallback-input.json'
    path.write_text(json.dumps([row]) + '\n')
    run = subprocess.run([str(work / 'fallback-probe'), 'render', 'core-markdown', profile, str(path)],
                         cwd=comparison, text=True, capture_output=True, check=True)
    baseline = subprocess.check_output([str(work / 'baseline'), 'render', 'core-markdown', profile, str(path)], cwd=comparison, text=True)
    assert run.stdout == baseline, row['name']
    counts = collections.Counter(line.split()[1] for line in run.stderr.splitlines() if line.startswith('FALLBACK '))
    fallback.append(dict(document=row['name'], profile=profile, bytes=len(row['markdown'].encode()),
                         input_sha256=hashlib.sha256(row['markdown'].encode()).hexdigest(),
                         fallback_counts=dict(counts), output_unchanged=True))
(out / 'fallback-frequency.json').write_text(json.dumps(fallback, indent=2) + '\n')
print(json.dumps(fallback, indent=2), flush=True)
